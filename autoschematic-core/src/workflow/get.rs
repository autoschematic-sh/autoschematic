use std::path::Path;

use crate::{
    config::AutoschematicConfig,
    connector::{FilterOutput, parse::connector_shortname},
    connector_cache::ConnectorCache,
    error::AutoschematicError,
    keystore::KeyStore,
};

pub async fn get(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<&Box<dyn KeyStore>>,
    prefix: &Path,
    addr: &Path,
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
                keystore,
            )
            .await?;

        if connector.filter(addr).await? == FilterOutput::Resource {
            if let Some(body) = connector.get(addr).await? {
                return Ok(Some(body.resource_definition));
            }
        }
    }

    Ok(None)
}
