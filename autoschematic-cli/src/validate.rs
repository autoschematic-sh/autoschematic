use anyhow::Context;
use autoschematic_core::{config::AutoschematicConfig, util::{repo_root, RON}};


pub fn validate() -> anyhow::Result<()> {
    let config_path = repo_root()?.join("autoschematic.ron");

    let _config: AutoschematicConfig =
        RON.from_str(&std::fs::read_to_string(&config_path).context("Reading autochematic.ron")?)
            .context("Parsing autoschematic.ron")?;

    Ok(())
}
