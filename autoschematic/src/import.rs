use std::path::PathBuf;


use autoschematic_core::connector_cache::ConnectorCache;

use crate::config::load_autoschematic_config;

pub async fn import(
    prefix: Option<String>,
    connector: Option<String>,
    subpath: Option<String>,
    overwrite: bool,
) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let connector_cache = ConnectorCache::default();

    let subpath = subpath.map(PathBuf::from);

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
