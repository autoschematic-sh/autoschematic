use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{config::AutoschematicConfig, connector_cache::ConnectorCache, error::AutoschematicError, keystore::KeyStore};

pub async fn list(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    prefix: &Path,
    connector_filter: &str,
    subpath: &Path,
) -> Result<Vec<PathBuf>, AutoschematicError> {
    let Some(prefix_name) = prefix.to_str() else {
        return Ok(Vec::new());
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_name) else {
        return Ok(Vec::new());
    };

    for connector_def in &prefix_def.connectors {
        if connector_def.shortname != connector_filter {
            continue;
        }

        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(&autoschematic_config, &prefix_name, &connector_def, keystore, true)
            .await?;

        let res = connector.list(subpath).await?;
        return Ok(res);
    }

    Ok(Vec::new())
}
