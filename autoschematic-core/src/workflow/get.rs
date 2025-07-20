use std::{path::Path, sync::Arc};

use crate::{
    config::AutoschematicConfig,
    connector::FilterResponse,
    connector_cache::ConnectorCache,
    error::AutoschematicError,
    keystore::KeyStore,
};

pub async fn get(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    prefix: &Path,
    virt_addr: &Path,
) -> Result<Option<Vec<u8>>, AutoschematicError> {
    let Some(prefix_str) = prefix.to_str() else {
        return Ok(None);
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_str) else {
        return Ok(None);
    };

    for connector_def in &prefix_def.connectors {
        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(
                &connector_def.shortname,
                &connector_def.spec,
                prefix,
                &connector_def.env,
                keystore.clone(),
            )
            .await?;

        if connector.filter(virt_addr).await? == FilterResponse::Resource {
            match connector.addr_virt_to_phy(virt_addr).await? {
                crate::connector::VirtToPhyResponse::NotPresent => {
                    return Ok(None);
                }
                crate::connector::VirtToPhyResponse::Deferred(read_outputs) => {
                    return Ok(None);
                }
                crate::connector::VirtToPhyResponse::Present(phy_addr) => {
                    if let Some(body) = connector.get(&phy_addr).await? {
                        return Ok(Some(body.resource_definition));
                    }
                }
                crate::connector::VirtToPhyResponse::Null(virt_addr) => {
                    if let Some(body) = connector.get(&virt_addr).await? {
                        return Ok(Some(body.resource_definition));
                    }
                }
            }
        }
    }

    Ok(None)
}
