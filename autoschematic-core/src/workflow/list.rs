use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{
    config::AutoschematicConfig,
    connector::{FilterOutput, parse::connector_shortname},
    connector_cache::ConnectorCache,
    error::AutoschematicError,
    keystore::KeyStore,
};

pub async fn list(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<&Box<dyn KeyStore>>,
    prefix: &Path,
    connector: &str,
    subpath: &Path,
) -> Result<Vec<PathBuf>, AutoschematicError> {
    let Some(prefix_str) = prefix.to_str() else {
        return Ok(Vec::new());
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_str) else {
        return Ok(Vec::new());
    };

    for connector_def in &prefix_def.connectors {
        let connector_shortname = connector_shortname(&connector_def.name)?;

        if connector_shortname != connector {
            continue;
        }

        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(&connector_def.name, prefix, &connector_def.env, keystore)
            .await?;

        let res = connector.list(subpath).await?;
        return Ok(res);
    }
    eprintln!("filter false");

    Ok(Vec::new())
}
