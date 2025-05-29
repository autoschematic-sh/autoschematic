use std::path::PathBuf;

use anyhow::bail;
use ron::ser::PrettyConfig;

use autoschematic_core::{
    config::AutoschematicConfig,
    connector_cache::{self, ConnectorCache},
    git_util::get_staged_files,
    util::{RON, repo_root},
};

use crate::{config::load_autoschematic_config, ui};

pub async fn import(
    prefix: Option<String>,
    connector: Option<String>,
    subpath: Option<String>,
    overwrite: bool,
) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let connector_cache = ConnectorCache::default();

    let subpath = subpath.map(|s| PathBuf::from(s));

    let keystore = None;

    eprintln!("Starting import. This may take a while!");

    let plan_report = autoschematic_core::workflow::import::import_all(
        &config,
        &connector_cache,
        keystore,
        subpath,
        prefix,
        connector,
        overwrite,
    )
    .await?;

    eprintln!("\u{1b}[32m Success! \u{1b}[39m");

    Ok(())
}
