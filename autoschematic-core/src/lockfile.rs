use std::{collections::HashMap, path::PathBuf};

use anyhow::bail;
use ron_pfnsec_fork as ron;
use serde::{Deserialize, Serialize};

use crate::{
    binary_cache::BinaryCache,
    connector::r#type::ConnectorType,
    error::{self, AutoschematicErrorType},
    manifest::ConnectorManifest,
    util,
};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AutoschematicLockfile {
    pub entries: HashMap<PathBuf, LockEntry>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct LockEntry {
    pub owner: String,
    pub repo: String,
    pub version: String,
    pub manifest: ConnectorManifest,
}

pub async fn load_lockfile() -> anyhow::Result<AutoschematicLockfile> {
    let repo_root = util::repo_root()?;
    let lockfile_path = repo_root.join("autoschematic.lock.ron");
    if !lockfile_path.is_file() {
        return Ok(AutoschematicLockfile::default());
    }
    let lockfile_s = tokio::fs::read_to_string(&lockfile_path).await?;

    let lockfile: AutoschematicLockfile = match ron::from_str(&lockfile_s) {
        Ok(it) => it,
        Err(err) => {
            return Err(error::AutoschematicError {
                kind: AutoschematicErrorType::InternalError(anyhow::anyhow!("Bad lockfile: {}", err)),
            }
            .into());
        }
    };
    Ok(lockfile)
}

pub async fn resolve_lock_entry(
    lockfile: &AutoschematicLockfile,
    connector_type: &ConnectorType,
    binary_cache: &BinaryCache,
) -> Result<Option<ConnectorType>, anyhow::Error> {
    match &connector_type {
        ConnectorType::LockFile(path, short_name) => {
            let Some(lock_entry) = lockfile.entries.get(path) else {
                return Err(error::AutoschematicError {
                    kind: AutoschematicErrorType::InternalError(anyhow::anyhow!(
                        "Lock entry {} not found",
                        path.to_string_lossy()
                    )),
                }
                .into());
            };
            match lock_entry.manifest.r#type.as_str() {
                "binary-tarpc" => {
                    binary_cache
                        .fetch_connector_release(
                            &lock_entry.owner,
                            &lock_entry.repo,
                            &lock_entry.version,
                            &lock_entry.manifest,
                            &util::short_target(),
                        )
                        .await?;
                    // TODO pick it up back here
                    Ok(None)
                }
                "python" => Ok(None),
                other => bail!("Connector Manifest has unrecognized type {}", other),
            }
        }
        _ => Ok(Some(connector_type.clone())),
    }
}
