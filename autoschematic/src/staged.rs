use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use git2::{Delta, DiffOptions, ErrorCode, FileMode, Oid, Repository};

/// Paths and blob IDs from the same index snapshot. A rename is a delete and an add.
pub struct StagedChanges {
    repo: Repository,
    entries: BTreeMap<PathBuf, Entry>,
}

enum Entry {
    Blob(Oid),
    Deleted,
    Unsupported(FileMode),
    Conflict,
}

impl StagedChanges {
    pub fn read(repo: Repository) -> anyhow::Result<Self> {
        let index = repo.index()?;
        let tree = match repo.head() {
            Ok(head) => Some(head.peel_to_tree()?),
            Err(e) if e.code() == ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e.into()),
        };
        let mut options = DiffOptions::new();
        options.include_typechange(true);
        let diff = repo.diff_tree_to_index(tree.as_ref(), Some(&index), Some(&mut options))?;
        let mut entries = BTreeMap::new();
        for delta in diff.deltas() {
            let file = delta.new_file();
            let path = file.path().context("Missing Git delta path")?;
            let entry = match delta.status() {
                Delta::Deleted => Entry::Deleted,
                Delta::Conflicted => Entry::Conflict,
                _ => match file.mode() {
                    FileMode::Blob | FileMode::BlobExecutable | FileMode::BlobGroupWritable => Entry::Blob(file.id()),
                    mode => Entry::Unsupported(mode),
                },
            };
            entries.insert(path.to_owned(), entry);
        }
        drop(diff);
        drop(tree);
        Ok(Self { repo, entries })
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        self.entries.keys().cloned().collect()
    }

    pub fn contents(&self, path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
        match self.entries.get(path).context("Missing staged path")? {
            Entry::Blob(id) => Ok(Some(self.repo.find_blob(*id)?.content().to_vec())),
            Entry::Deleted => Ok(None),
            Entry::Conflict => bail!("Unresolved merge conflict at {}", path.display()),
            Entry::Unsupported(mode) => bail!("Unsupported staged file mode {mode:?} at {}", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_bytes_survive_worktree_edits_restaging_and_recreated_deletions() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let path = Path::new("resource");
        let file = dir.path().join(path);
        std::fs::write(&file, [0xff, 0]).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(path).unwrap();
        index.write().unwrap();
        let snapshot = StagedChanges::read(Repository::open(dir.path()).unwrap()).unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "baseline", &tree, &[]).unwrap();
        std::fs::write(&file, "new worktree bytes").unwrap();
        index.add_path(path).unwrap();
        index.write().unwrap();
        assert_eq!(snapshot.contents(path).unwrap().unwrap(), [0xff, 0]);
        index.remove_path(path).unwrap();
        index.write().unwrap();
        let deleted = StagedChanges::read(Repository::open(dir.path()).unwrap()).unwrap();
        assert_eq!(deleted.paths(), [path]);
        assert!(deleted.contents(path).unwrap().is_none());
    }
}
