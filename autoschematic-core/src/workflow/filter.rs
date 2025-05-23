use std::path::Path;

use crate::{
    config::AutoschematicConfig,
    connector::{FilterOutput, parse::connector_shortname},
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
        let _connector_shortname = connector_shortname(&connector_def.name)?;

        let (connector, _inbox) = connector_cache
            .get_or_spawn_connector(&connector_def.name, prefix, &connector_def.env, keystore)
            .await?;

        match connector.filter(addr).await? {
            FilterOutput::Config => return Ok(FilterOutput::Config),
            FilterOutput::Resource => return Ok(FilterOutput::Resource),
            FilterOutput::None => continue,
        }
    }

    Ok(FilterOutput::None)
}
