use std::{path::Path, sync::Arc};

use documented::{Documented, DocumentedFields};

use crate::{
    config::{AutoschematicConfig, Connector, Prefix},
    connector::{DocIdent, FilterResponse, GetDocResponse},
    connector_cache::ConnectorCache,
    error::AutoschematicError,
    keystore::KeyStore,
};

pub fn get_system_docstring(path: &Path, ident: DocIdent) -> Result<Option<GetDocResponse>, AutoschematicError> {
    let Some(path) = path.to_str() else {
        return Ok(None);
    };

    match path {
        "autoschematic.ron" => match ident {
            DocIdent::Struct { name } => match name.as_str() {
                "AutoschematicConfig" => Ok(Some(AutoschematicConfig::DOCS.into())),
                "Prefix" => Ok(Some(Prefix::DOCS.into())),
                "Connector" => Ok(Some(Connector::DOCS.into())),
                _ => Ok(None),
            },
            DocIdent::Field { parent, name } => match parent.as_str() {
                "AutoschematicConfig" => Ok(Some(AutoschematicConfig::get_field_docs(name)?.into())),
                "Prefix" => Ok(Some(Prefix::get_field_docs(name)?.into())),
                "Connector" => Ok(Some(Connector::get_field_docs(name)?.into())),
                _ => Ok(None),
            },
        },
        // "autoschematic.rbac.ron" => match ident {
        //     DocIdent::Struct { name } => todo!(),
        //     DocIdent::Field { parent, name } => todo!(),
        // },
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
