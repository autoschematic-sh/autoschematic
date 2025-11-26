use anyhow::Context;
use autoschematic_core::{
    config::AutoschematicConfig,
    config_rbac::AutoschematicRbacConfig,
    util::{RON, repo_root},
};

pub fn validate() -> anyhow::Result<()> {
    let config_path = repo_root()?.join("autoschematic.ron");

    let _config: AutoschematicConfig = RON
        .from_str(&std::fs::read_to_string(&config_path).context("Reading autoschematic.ron")?)
        .context("Parsing autoschematic.ron")?;

    let rbac_config_path = repo_root()?.join("autoschematic.ron");
    if rbac_config_path.is_file() {
        let _rbac_config: AutoschematicRbacConfig = RON
            .from_str(&std::fs::read_to_string(&config_path).context("Reading autoschematic.rbac.ron")?)
            .context("Parsing autoschematic.rbac.ron")?;
    }

    Ok(())
}
