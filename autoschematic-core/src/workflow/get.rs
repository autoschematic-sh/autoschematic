use std::{ffi::OsString, path::Path};

use crate::{
    config::AutoschematicConfig, connector::parse::connector_shortname, connector_cache::ConnectorCache,
    error::AutoschematicError, keystore::KeyStore,
};

pub async fn get(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<&Box<dyn KeyStore>>,
    prefix: &Path,
    addr: &Path,
) -> Result<Option<OsString>, AutoschematicError> {
    let Some(prefix_str) = prefix.to_str() else {
        return Ok(None);
    };
    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_str) else {
        return Ok(None);
    };

    for connector_def in &prefix_def.connectors {
        let _connector_shortname = connector_shortname(&connector_def.name)?;

        let (connector, _inbox) = connector_cache
            .get_or_init(&connector_def.name, prefix, &connector_def.env, keystore)
            .await?;

        if connector.filter(addr).await? {
            eprintln!("filter true");
            if let Some(body) = connector.get(addr).await? {
                return Ok(Some(body.resource_definition));
            }
        }
    }
    eprintln!("filter false");

    Ok(None)
}
