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
    let Some(prefix_str) = prefix.to_str() else {
        return Ok(Vec::new());
    };
    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_str) else {
        return Ok(Vec::new());
    };

    let (connector, _inbox) = connector_cache
        .get_or_spawn_connector(
            &connector_def.shortname,
            &connector_def.spec,
            prefix,
            &connector_def.env,
            keystore.clone(),
            false,
        )
        .await?;

    let skeletons = connector.get_skeletons().await?;

    Ok(skeletons)
}
