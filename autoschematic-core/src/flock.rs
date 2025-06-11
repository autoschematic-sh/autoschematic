use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

// use nix::fcntl::{Flock, FlockArg};
// use tokio::task::spawn_blocking;

// pub fn hold_flock_blocking(path: &Path) -> anyhow::Result<Flock<File>> {
//     let file = OpenOptions::new().create(true).truncate(true).write(true).open(path)?;

//     let lock = match Flock::lock(file, FlockArg::LockExclusive) {
//         Ok(l) => l,
//         Err((_, e)) => return Err(e.into()),
//     };

//     Ok(lock)
// }

// pub async fn wait_for_flock(path: PathBuf) -> anyhow::Result<Flock<File>> {
//     spawn_blocking(move || -> Result<Flock<File>, anyhow::Error> { hold_flock_blocking(&path) }).await?
// }
