use std::{io::Write, sync::Arc};

use autoschematic_core::{
    connector_cache::ConnectorCache, git_util::get_staged_files, report::PlanReport, workflow::unbundle::write_unbundle_element,
};
use crossterm::style::Stylize;

use crate::{
    config::load_autoschematic_config,
    spinner::spinner::show_spinner,
    util::{colour_op_message, try_colour_op_message_diff},
};

pub async fn unbundle(
    prefix: &Option<String>,
    connector: &Option<String>,
    subpath: &Option<String>,
    overbundle: bool,
    git_stage: bool,
) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let staged_files = get_staged_files()?;

    let connector_cache = Arc::new(ConnectorCache::default());

    let keystore = None;

    if staged_files.is_empty() {
        println!(" ∅  No files staged in git. Stage modified files with `git add` to unbundle them.");
        return Ok(());
    }

    let mut have_nonempty_unbundle = false;
    for path in staged_files {
        let spinner_stop = show_spinner().await;

        let Some(unbundle_report) = autoschematic_core::workflow::unbundle::unbundle(
            &config,
            connector_cache.clone(),
            keystore.clone(),
            connector,
            &path,
        )
        .await?
        else {
            spinner_stop.send(()).unwrap();
            continue;
        };

        // println!("{plan_report:#?}");

        have_nonempty_unbundle = true;

        spinner_stop.send(()).unwrap();

        let Some(unbundle_elements) = unbundle_report.elements else {
            continue;
        };

        if unbundle_elements.is_empty() {
            continue;
        }

        println!(
            " {}/{}:",
            unbundle_report.prefix.display().to_string().dark_grey(),
            unbundle_report.addr.display().to_string().bold()
        );

        for element in &unbundle_elements {
            write_unbundle_element(&unbundle_report.prefix, &unbundle_report.addr, element, overbundle, git_stage).await?;
            println!(
                " ↪ {}/{}:",
                unbundle_report.prefix.display().to_string().dark_grey(),
                element.addr.display().to_string().underline_dark_grey()
            );
        }
    }

    Ok(())
}
