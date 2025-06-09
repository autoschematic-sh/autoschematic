
use anyhow::Context;
use autoschematic_core::{config::AutoschematicConfig, util::{repo_root, RON}};

pub fn load_autoschematic_config() -> anyhow::Result<AutoschematicConfig> {
    let repo_root = repo_root()?;
    let config_path = repo_root.join("autoschematic.ron");
    let config_body = std::fs::read_to_string(config_path).context("Reading autoschematic.ron")?;
    let config_file: AutoschematicConfig = RON.from_str(&config_body).context("Parsing autoschematic.ron")?;
    
    Ok(config_file.into())
}