use std::path::PathBuf;

use anyhow::bail;
use autoschematic_core::{
    util::{load_autoschematic_config, split_prefix_addr},
    workflow,
};

use crate::CONNECTOR_CACHE;

pub async fn check_drift(path: &str) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;
    let Some((prefix, addr)) = split_prefix_addr(&config, &PathBuf::from(path)) else {
        CONNECTOR_CACHE.clear().await;
        bail!("Not an address for any active connector");
    };

    match workflow::check_drift::check_drift(&config, &CONNECTOR_CACHE, None, &prefix, &addr).await? {
        workflow::check_drift::CheckDriftResult::NeitherExist => Ok(()),
        workflow::check_drift::CheckDriftResult::InvalidAddress => bail!("Not an address for any active connector"),
        workflow::check_drift::CheckDriftResult::NotEqual { .. } => bail!("Resource has drifted"),
        workflow::check_drift::CheckDriftResult::Equal => Ok(()),
    }
}
