use anyhow::bail;
use ron::ser::PrettyConfig;

use autoschematic_core::{
    config::AutoschematicConfig,
    connector_cache::{self, ConnectorCache},
    git_util::{get_staged_files, git_add, git_commit},
    util::{repo_root, RON},
};

use crate::{config::load_autoschematic_config, ui};

pub async fn apply(prefix: &Option<String>, connector: &Option<String>, subpath: &Option<String>) -> anyhow::Result<()> {
    let repo_root = repo_root()?;
    let config = load_autoschematic_config()?;

    let staged_files = get_staged_files()?;

    println!("{:?}", staged_files);

    let connector_cache = ConnectorCache::default();

    let keystore = None;

    let mut do_commit = false;
    for path in staged_files {
        if let Some(plan_report) =
            autoschematic_core::workflow::plan::plan(&config, &connector_cache, keystore, &connector, &path).await?
        {
            if let Some(apply_report) =
                autoschematic_core::workflow::apply::apply(&config, &connector_cache, keystore, &connector, &plan_report)
                    .await?
            {
                println!("{:?}", apply_report);
                for path in &apply_report.wrote_files {
                    git_add(&repo_root, path)?;
                }
                do_commit = true;
            }
        }
    }
    if do_commit {
        git_commit(&repo_root, "autoschematic", "apply@autoschematic.sh", "autoschematic apply")?;
    }

    // ui::main();

    Ok(())
}
