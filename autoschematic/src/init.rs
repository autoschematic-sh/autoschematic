use anyhow::bail;
use ron::ser::PrettyConfig;
use ron_pfnsec_fork as ron;

use autoschematic_core::{
    config::AutoschematicConfig,
    config_rbac::AutoschematicRbacConfig,
    util::{RON, repo_root},
};

pub fn init() -> anyhow::Result<()> {
    let config_path = repo_root()?.join("autoschematic.ron");
    if config_path.is_file() {
        bail!("Autoschematic config already exists at autoschematic.ron. Delete it before running `autoschematic init`.")
    }

    let pretty_config = PrettyConfig::default().struct_names(true);
    let config = AutoschematicConfig::default();
    let s = RON.to_string_pretty(&config, pretty_config)?;
    std::fs::write(config_path, &s)?;

    Ok(())
}

pub fn init_rbac() -> anyhow::Result<()> {
    let config_path = repo_root()?.join("autoschematic.rbac.ron");
    if config_path.is_file() {
        bail!(
            "Autoschematic RBAC config already exists at autoschematic.rbac.ron. Delete it before running `autoschematic init rbac`."
        )
    }

    let pretty_config = PrettyConfig::default().struct_names(true);
    let config = AutoschematicRbacConfig::default();
    let s = RON.to_string_pretty(&config, pretty_config)?;
    std::fs::write(config_path, &s)?;

    Ok(())
}
