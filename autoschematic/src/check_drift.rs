use std::path::PathBuf;

use anyhow::bail;
use autoschematic_core::{
    util::{load_autoschematic_config, split_prefix_addr},
    workflow,
};

use crate::CONNECTOR_CACHE;

pub async fn check_drift(path: &str, connector_filter: Option<&str>) -> anyhow::Result<()> {
    let mut config = load_autoschematic_config()?;
    let Some((prefix, addr)) = split_prefix_addr(&config, &PathBuf::from(path)) else {
        CONNECTOR_CACHE.clear().await;
        bail!("Not an address for any active connector");
    };

    if let Some(name) = connector_filter {
        let Some(prefix_def) = prefix.to_str().and_then(|prefix| config.prefixes.get_mut(prefix)) else {
            bail!("Unknown prefix: {}", prefix.display());
        };
        prefix_def.connectors.retain(|connector| connector.shortname == name);
        if prefix_def.connectors.is_empty() {
            bail!("No connector named {name} in prefix {}", prefix.display());
        }
    }

    match workflow::check_drift::check_drift(&config, &CONNECTOR_CACHE, None, &prefix, &addr).await? {
        workflow::check_drift::CheckDriftResult::NeitherExist => Ok(()),
        workflow::check_drift::CheckDriftResult::InvalidAddress => bail!("Not an address for any active connector"),
        workflow::check_drift::CheckDriftResult::NotEqual { .. } => bail!("Resource has drifted"),
        workflow::check_drift::CheckDriftResult::Equal => Ok(()),
    }
}
