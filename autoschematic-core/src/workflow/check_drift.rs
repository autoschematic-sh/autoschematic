use std::{path::Path, sync::Arc};

use crate::{
    config::AutoschematicConfig, connector::FilterResponse, connector_cache::ConnectorCache, error::AutoschematicError,
    keystore::KeyStore,
};

pub enum CheckDriftResult {
    /// Neither the desired (local) resource nor the current (remote) resource exist.
    NeitherExist,
    /// The address provided doesn't correspond to a resource file for any loaded connector
    InvalidAddress,
    /// Both exist, and have drifted; Connector::eq(...) returned false.
    NotEqual {
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    },
    /// Both exist, and Connector::eq(...) returned true.
    Equal,
}

pub async fn check_drift(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    prefix: &Path,
    virt_addr: &Path,
) -> Result<CheckDriftResult, AutoschematicError> {
    let Some(prefix_name) = prefix.to_str() else {
        return Ok(CheckDriftResult::InvalidAddress);
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_name) else {
        return Ok(CheckDriftResult::InvalidAddress);
    };

    let desired_path = prefix.join(virt_addr);

    let desired_state = if desired_path.is_file() {
        Some(tokio::fs::read_to_string(desired_path).await?)
    } else {
        None
    };

    let mut current_state = None;

    for connector_def in &prefix_def.connectors {
        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(autoschematic_config, prefix_name, connector_def, keystore.clone(), true)
            .await?;

        if connector.filter(virt_addr).await? == FilterResponse::Resource {
            match connector.addr_virt_to_phy(virt_addr).await? {
                crate::connector::VirtToPhyResponse::NotPresent => {
                    current_state = None;
                }
                crate::connector::VirtToPhyResponse::Deferred(_read_outputs) => {
                    current_state = None;
                }
                crate::connector::VirtToPhyResponse::Present(phy_addr) => {
                    if let Some(body) = connector.get(&phy_addr).await? {
                        // return Ok(Some(body.resource_definition));
                        // TODO we also need to check if outputs have drifted!
                        current_state = Some(body.resource_definition);
                    }
                }
                crate::connector::VirtToPhyResponse::Null(virt_addr) => {
                    if let Some(body) = connector.get(&virt_addr).await? {
                        current_state = Some(body.resource_definition);
                        // return Ok(Some(body.resource_definition));
                    }
                }
            }

            match (current_state, desired_state) {
                (None, None) => return Ok(CheckDriftResult::NeitherExist),
                (None, Some(b)) => {
                    return Ok(CheckDriftResult::NotEqual {
                        current: None,
                        desired: Some(b.into()),
                    });
                }
                (Some(a), None) => {
                    return Ok(CheckDriftResult::NotEqual {
                        current: Some(a),
                        desired: None,
                    });
                }
                (Some(a), Some(b)) => match connector.eq(virt_addr, &a, b.as_bytes()).await? {
                    true => return Ok(CheckDriftResult::Equal),
                    false => {
                        return Ok(CheckDriftResult::NotEqual {
                            current: Some(a),
                            desired: Some(b.into()),
                        });
                    }
                },
            }
        }
    }

    Ok(CheckDriftResult::InvalidAddress)
}
