use std::{path::Path, sync::Arc};

use crate::{
    config::AutoschematicConfig,
    connector::{DocIdent, FilterResponse, GetDocResponse},
    connector_cache::ConnectorCache,
    error::AutoschematicError,
    keystore::KeyStore,
};

pub async fn get_docstring(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    prefix: &Path,
    addr: &Path,
    ident: DocIdent,
) -> Result<Option<GetDocResponse>, AutoschematicError> {
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
                false,
            )
            .await?;

        if connector.filter(addr).await?.intersects(FilterResponse::none())
            && let Some(doc) = connector.get_docstring(addr, ident.clone()).await?
        {
            return Ok(Some(doc));
        }
    }

    Ok(None)
}
