
use anyhow::bail;
use ron::ser::PrettyConfig;

use autoschematic_core::{config::AutoschematicConfig, util::{repo_root, RON}};

pub fn init() -> anyhow::Result<()> {
    let config_path = repo_root()?.join("autoschematic.ron");
    if config_path.is_file() {
        bail!("Autoschematic config already exists at autoschematic.ron. Delete it before running `autoschematic init`.")
    }
    
    let pretty_config = PrettyConfig::default()
        .struct_names(true);
    let config = AutoschematicConfig::default();
    let s = RON.to_string_pretty(&config, pretty_config)?;
    std::fs::write(config_path, &s)?;

    Ok(())
}
