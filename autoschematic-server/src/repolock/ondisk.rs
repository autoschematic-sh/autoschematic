use std::path::{Path, PathBuf};

use anyhow::bail;
use file_lock::{FileLock, FileOptions};

use super::{RepoLock, RepoLockStore};



#[derive(Default, Debug)]
pub struct OnDiskLockStore {
    dir: PathBuf
}

pub struct OnDiskLock {
    lock: FileLock
}

impl RepoLockStore for OnDiskLockStore {
    fn new(path: &Path) -> anyhow::Result<Self> where Self: Sized {
        if !path.is_dir() {
            bail!("OnDiskLockStore: path must be a directory")
        }
        Ok(Self {
            dir: path.into()
        })
    }

    fn try_lock(&self, path: &Path) -> anyhow::Result<Box<dyn super::RepoLock>> {
        let options = FileOptions::new()
        .write(true)
        .create(true)
        .append(true);

        let filelock = FileLock::lock(self.dir.join(path), false, options)?;

        let lock = OnDiskLock {
            lock: filelock
        };
        
        return Ok(Box::new(lock));
    }
}

impl RepoLock for OnDiskLock {
    fn unlock(&self) {
        if let Err(e) = self.lock.unlock() {
            tracing::error!("Failed to unlock repo: {}", e);
        }
    }
}