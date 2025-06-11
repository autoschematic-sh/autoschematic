use autoschematic_core::{
    connector_cache::ConnectorCache,
    git_util::get_staged_files,
};

use crate::config::load_autoschematic_config;

pub async fn plan(prefix: &Option<String>, connector: &Option<String>, subpath: &Option<String>) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let staged_files = get_staged_files()?;

    println!("{:?}", staged_files);

    let connector_cache = ConnectorCache::default();

    let keystore = None;

    for path in staged_files {
        let plan_report =
            autoschematic_core::workflow::plan::plan(&config, &connector_cache, keystore, connector, &path).await?;

        println!("{:?}", plan_report);
    }

    // ui::main();

    Ok(())
}
