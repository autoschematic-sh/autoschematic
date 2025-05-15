use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};

use anyhow::bail;
use tokio::sync::{Mutex, MutexGuard};

use super::{RepoLock, RepoLockStore};



#[derive(Default)]
pub struct InMemLockStore {
    lock_map: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>
}

pub struct InMemLock<'a> {
    guard: Arc<MutexGuard<'a, ()>>
}

impl RepoLockStore for InMemLockStore {
    fn new(path: &Path) -> anyhow::Result<Self> where Self: Sized {
        Ok(Self::default())
    }

    fn try_lock(&self, path: &Path) -> anyhow::Result<Box<dyn super::RepoLock + '_>> {
        let mut lock_map_res = self.lock_map.try_lock();
        match lock_map_res {
            Ok(mut lock_map) => {
                if !lock_map.contains_key(path) {
                    lock_map.insert(PathBuf::from(path), Arc::new(Mutex::new(())));
                }
                let lock_res = lock_map.get(path).unwrap().try_lock();
                match lock_res{
                    Ok(lock) => {
                        let lock_arc = Arc::new(lock);
                        Ok(Box::new(InMemLock {
                            guard: lock_arc.clone()
                        }))
                    }
                    Err(e) => {
                        bail!("Failed to lock")
                    }
                }
            }
            Err(e) => {
                bail!("Failed to lock")
            }
        }
    }
}

impl RepoLock for InMemLock<'_> {
    fn unlock(&self) {
        drop(&self.guard)
    }
}