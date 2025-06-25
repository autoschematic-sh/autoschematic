use std::path::Path;

use crate::{
    config::AutoschematicConfig,
    connector::FilterOutput,
    connector_cache::ConnectorCache,
    keystore::KeyStore,
};

pub async fn filter(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<&Box<dyn KeyStore>>,
    prefix: &Path,
    addr: &Path,
) -> anyhow::Result<FilterOutput> {
    let Some(prefix_str) = prefix.to_str() else {
        return Ok(FilterOutput::None);
    };
    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix_str) else {
        return Ok(FilterOutput::None);
    };

    for connector_def in &prefix_def.connectors {
        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(
                &connector_def.shortname,
                &connector_def.spec,
                prefix,
                &connector_def.env,
                keystore,
            )
            .await?;

        match connector.filter(addr).await? {
            FilterOutput::Config => return Ok(FilterOutput::Config),
            FilterOutput::Resource => return Ok(FilterOutput::Resource),
            FilterOutput::Bundle => return Ok(FilterOutput::Bundle),
            FilterOutput::None => continue,
        }
    }

    Ok(FilterOutput::None)
}
