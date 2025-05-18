use std::path::Path;

use crate::{
    config::AutoschematicConfig, connector::parse::connector_shortname, connector_cache::ConnectorCache, keystore::KeyStore,
};

pub async fn filter(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<&Box<dyn KeyStore>>,
    prefix: &Path,
    addr: &Path,
) -> anyhow::Result<bool> {
    let Some(prefix_str) = prefix.to_str() else {
        return Ok(false);
    };
    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_str) else {
        return Ok(false);
    };

    for connector_def in &prefix_def.connectors {
        let _connector_shortname = connector_shortname(&connector_def.name)?;

        let (connector, _inbox) = connector_cache
            .get_or_init(&connector_def.name, prefix, &connector_def.env, keystore)
            .await?;

        if connector.filter(addr).await? {
            return Ok(true);
        }
        //     if let Some(body) = connector.get(addr).await? {
        //         return Ok(Some(body.resource_definition));
        //     }
        // }
    }

    Ok(false)
}
