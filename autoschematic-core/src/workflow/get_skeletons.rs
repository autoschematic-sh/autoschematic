use std::{path::Path, sync::Arc};

use crate::{
    config, config::AutoschematicConfig, connector::SkeletonResponse, connector_cache::ConnectorCache,
    error::AutoschematicError, keystore::KeyStore,
};

pub async fn get_skeletons(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    prefix: &Path,
    connector_def: &config::Connector,
) -> Result<Vec<SkeletonResponse>, AutoschematicError> {
    let Some(prefix_name) = prefix.to_str() else {
        return Ok(Vec::new());
    };

    let Some(_prefix_def) = autoschematic_config.prefixes.get(prefix_name) else {
        return Ok(Vec::new());
    };

    let (connector, _inbox) = connector_cache
        .get_or_spawn_connector(&autoschematic_config, prefix_name, &connector_def, keystore.clone(), false)
        .await?;

    let skeletons = connector.get_skeletons().await?;

    Ok(skeletons)
}
