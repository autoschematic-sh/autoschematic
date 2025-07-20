use std::{path::Path, sync::Arc};

use crate::{config::AutoschematicConfig, connector::FilterResponse, connector_cache::ConnectorCache, keystore::KeyStore};

pub async fn filter(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    connector_filter: Option<&str>,
    prefix: &Path,
    addr: &Path,
) -> anyhow::Result<FilterResponse> {
    let Some(prefix_str) = prefix.to_str() else {
        return Ok(FilterResponse::None);
    };
    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_str) else {
        return Ok(FilterResponse::None);
    };

    for connector_def in &prefix_def.connectors {
        if connector_filter.is_some_and(|f| f != connector_def.shortname) {
            continue;
        }
        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(
                &connector_def.shortname,
                &connector_def.spec,
                prefix,
                &connector_def.env,
                keystore.clone(),
            )
            .await?;

        match connector.filter(addr).await? {
            FilterResponse::Config => return Ok(FilterResponse::Config),
            FilterResponse::Resource => return Ok(FilterResponse::Resource),
            FilterResponse::Bundle => return Ok(FilterResponse::Bundle),
            FilterResponse::Task => return Ok(FilterResponse::Task),
            FilterResponse::None => continue,
        }
    }

    Ok(FilterResponse::None)
}
