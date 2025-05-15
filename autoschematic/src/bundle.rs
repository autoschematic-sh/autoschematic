use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;

use crate::error::{AutoschematicServerError, AutoschematicServerErrorType};

// #[cfg(feature = "python")]
// pub mod python;

pub struct BundleOutput {
    pub filename: PathBuf,
    pub file_contents: String
}

#[async_trait]
pub trait Bundle: Send + Sync {
    // Attempt to instantiate a Connector mounted at `prefix` from environment variables, config files, etc.
    async fn new(
        name: &str,
        prefix: &Path,
    ) -> Result<Box<dyn Bundle>, anyhow::Error>
    where
        Self: Sized;

    // For all files affected by the PR, this filter determines if the bundle cares about them
    // (E.G. README.md -> false, cluster.json -> true)
    // In essence, this decides on the subset of the address space that the bundle
    // will manage, where "address space" is the nested hierarchy of files.
    // If `addr` falls within the address space of this bundle, return true.
    fn filter(&self, addr: &Path) -> Result<bool, anyhow::Error>;

    async fn exec(&self, addr: &Path, resource: String) -> anyhow::Result<Vec<BundleOutput>>;
}

pub async fn bundle_init(
    name: &str,
    prefix: &Path,
) -> Result<Box<dyn Bundle>, AutoschematicServerError> {

    match name {
        other => {
            // E.G. python:modules/snowflake.py:SnowflakeConnector
            let re = Regex::new(r"^(?<type>[^:]+):(?<path>.+)$")?;

            let Some(caps) = re.captures(other) else {
                return Err(AutoschematicServerError {
                    kind: AutoschematicServerErrorType::InvalidConnectorString(name.to_string()),
                });
            };

            match &caps["type"] {
                // #[cfg(feature = "python")]
                // "python" => Ok((
                //     PythonBundle::new(&caps["path"], prefix, outbox).await?,
                // )),

                _ => Err(AutoschematicServerError {
                    kind: AutoschematicServerErrorType::InvalidConnectorString(name.to_string()),
                }),
            }
        }
    }
}
