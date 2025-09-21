#![allow(unused)]
pub mod ondisk;
// pub mod inmem;

use std::path::{Path, PathBuf};

use crate::error::{AutoschematicServerError, AutoschematicServerErrorType};
use anyhow::Result;
use ondisk::OnDiskLockStore;
use regex::Regex;

pub trait RepoLockStore: Send + Sync + std::fmt::Debug {
    fn new(path: &Path) -> Result<Self>
    where
        Self: Sized;
    fn try_lock(&self, path: &Path) -> Result<Box<dyn RepoLock>>;
}
pub trait RepoLock {
    fn unlock(&self);
}

pub fn repolockstore_init(name: &str) -> Result<Box<dyn RepoLockStore>> {
    let re = Regex::new(r"^(?<type>[^:/]+)://(?<path>.+)$")?;

    let Some(caps) = re.captures(name) else {
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::InvalidConnectorString(name.to_string()),
        }
        .into());
    };

    match &caps["type"] {
        "ondisk" => Ok(Box::new(OnDiskLockStore::new(&PathBuf::from(&caps["path"]))?)),
        // "inmem" => Ok(Box::new(InMemLockStore::new(&PathBuf::from(&caps["path"]))?)),
        _ => Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::InvalidConnectorString(name.to_string()),
        }
        .into()),
    }
}
