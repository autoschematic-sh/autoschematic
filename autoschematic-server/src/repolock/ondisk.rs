use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use anyhow::bail;
use file_guard::{FileGuard, Lock};

use super::{RepoLock, RepoLockStore};

#[derive(Default, Debug)]
pub struct OnDiskLockStore {
    dir: PathBuf,
}

pub struct OnDiskLock {
    lock: FileGuard<Rc<File>>,
}

impl RepoLockStore for OnDiskLockStore {
    fn new(path: &Path) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        if !path.is_dir() {
            bail!("OnDiskLockStore: path must be a directory")
        }
        Ok(Self { dir: path.into() })
    }

    fn try_lock(&self, path: &Path) -> anyhow::Result<Box<dyn super::RepoLock>> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(self.dir.join(path))?;

        let guard = file_guard::lock(Rc::new(file), Lock::Exclusive, 0, 1)?;

        // let filelock = FileLock::lock(self.dir.join(path), false, options)?;

        let lock = OnDiskLock { lock: guard };

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
