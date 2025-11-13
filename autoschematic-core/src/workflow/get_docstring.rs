use std::{path::Path, sync::Arc};

use crate::{
    config::{AutoschematicConfig, AuxTask, Connector, Prefix},
    config_rbac::{self},
    connector::{DocIdent, FilterResponse, GetDocResponse},
    connector_cache::ConnectorCache,
    doc_dispatch,
    error::AutoschematicError,
    keystore::KeyStore,
};

pub fn get_system_docstring(path: &Path, ident: DocIdent) -> Result<Option<GetDocResponse>, AutoschematicError> {
    let Some(path) = path.to_str() else {
        return Ok(None);
    };

    match path {
        "autoschematic.ron" => doc_dispatch!(ident, [AutoschematicConfig, Prefix, Connector, AuxTask], [Spec]),
        "autoschematic.rbac.ron" => doc_dispatch!(
            ident,
            [
                config_rbac::AutoschematicRbacConfig,
                config_rbac::Role,
                config_rbac::PrefixGrant
            ],
            [config_rbac::User, config_rbac::Grant]
        ),
        _ => Ok(None),
    }
}

// TODO This is trivially cacheable!
pub async fn get_docstring(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    prefix: &Path,
    addr: &Path,
    ident: DocIdent,
) -> Result<Option<GetDocResponse>, AutoschematicError> {
    let Some(prefix_name) = prefix.to_str() else {
        return Ok(None);
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_name) else {
        return Ok(None);
    };

    for connector_def in &prefix_def.connectors {
        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(autoschematic_config, prefix_name, connector_def, keystore.clone(), false)
            .await?;

        if connector.filter(addr).await?.intersects(FilterResponse::none())
            && let Some(doc) = connector.get_docstring(addr, ident.clone()).await?
        {
            return Ok(Some(doc));
        }
    }

    Ok(None)
}
