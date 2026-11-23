use anyhow::Context;
use autoschematic_core::{
    config::AutoschematicConfig,
    config_rbac::AutoschematicRbacConfig,
    util::{RON, repo_root},
};
use std::path::Path;

pub fn validate() -> anyhow::Result<()> {
    validate_repo(&repo_root()?)
}

fn validate_repo(repo_root: &Path) -> anyhow::Result<()> {
    let config_path = repo_root.join("autoschematic.ron");

    let _config: AutoschematicConfig = RON
        .from_str(&std::fs::read_to_string(&config_path).context("Reading autoschematic.ron")?)
        .context("Parsing autoschematic.ron")?;

    let rbac_config_path = repo_root.join("autoschematic.rbac.ron");
    if rbac_config_path.is_file() {
        let _rbac_config: AutoschematicRbacConfig = RON
            .from_str(&std::fs::read_to_string(&rbac_config_path).context("Reading autoschematic.rbac.ron")?)
            .context("Parsing autoschematic.rbac.ron")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const CONFIG: &str = "AutoschematicConfig(prefixes: {})";
    const RBAC_CONFIG: &str = "AutoschematicRbacConfig(roles: {})";
    static NEXT_REPO_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestRepo(std::path::PathBuf);

    impl TestRepo {
        fn new() -> Self {
            let id = NEXT_REPO_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("autoschematic-validate-{}-{id}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn validates_without_optional_rbac_config() {
        let repo = TestRepo::new();
        std::fs::write(repo.path().join("autoschematic.ron"), CONFIG).unwrap();

        validate_repo(repo.path()).unwrap();
    }

    #[test]
    fn validates_rbac_config_from_its_own_path() {
        let repo = TestRepo::new();
        std::fs::write(repo.path().join("autoschematic.ron"), CONFIG).unwrap();
        std::fs::write(repo.path().join("autoschematic.rbac.ron"), RBAC_CONFIG).unwrap();

        validate_repo(repo.path()).unwrap();
    }

    #[test]
    fn reports_invalid_rbac_config() {
        let repo = TestRepo::new();
        std::fs::write(repo.path().join("autoschematic.ron"), CONFIG).unwrap();
        std::fs::write(repo.path().join("autoschematic.rbac.ron"), CONFIG).unwrap();

        let error = validate_repo(repo.path()).unwrap_err();
        assert!(format!("{error:#}").contains("Parsing autoschematic.rbac.ron"));
    }
}
