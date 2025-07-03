use std::sync::Arc;

use autoschematic_core::{connector_cache::ConnectorCache, git_util::get_staged_files, report::PlanReport};
use crossterm::style::Stylize;

use crate::{config::load_autoschematic_config, spinner::spinner::show_spinner, util::colour_op_message};
use colored::Colorize;

pub async fn plan(prefix: &Option<String>, connector: &Option<String>, subpath: &Option<String>) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let staged_files = get_staged_files()?;

    let connector_cache = Arc::new(ConnectorCache::default());

    let keystore = None;

    if staged_files.is_empty() {
        println!(" ∅  No files staged in git. Stage modified files with git add to plan or apply them.");
        return Ok(());
    }

    let mut need_print_frame_start = true;
    let mut need_print_frame_end = false;
    let mut have_nonempty_plan = false;
    for path in staged_files {
        let spinner_stop = show_spinner().await;

        let Some(plan_report) =
            autoschematic_core::workflow::plan::plan(&config, connector_cache.clone(), keystore.clone(), connector, &path)
                .await?
        else {
            spinner_stop.send(()).unwrap();
            continue;
        };
        
        // println!("{plan_report:#?}");

        have_nonempty_plan = true;

        spinner_stop.send(()).unwrap();

        if plan_report.connector_ops.is_empty() {
            continue;
        }

        if need_print_frame_start {
            need_print_frame_start = false;
            need_print_frame_end = true;
            print_frame_start();
        }

        print_plan(&plan_report);
    }

    if need_print_frame_end {
        print_frame_end();
    }

    println!(" ◇ Plan complete.");
    if !have_nonempty_plan {
        println!(
            " ≡ All plans are empty, implying that the remote configuration matches the desired configuration for all staged files."
        );
        return Ok(());
    }

    Ok(())
}

pub fn print_frame_start() {
    println!(
        "{}",
        "╔════════════════════════════════════════════════════════════════════════════════════════════════════╗".dark_grey()
    );
}

pub fn frame() -> String {
    "║".dark_grey().to_string()
}

pub fn print_frame_end() {
    println!(
        "{}",
        "╚════════════════════════════════════════════════════════════════════════════════════════════════════╝".dark_grey()
    );
}

pub fn print_plan_addr(plan_report: &PlanReport) {
    println!(
        "{} At {}/{}:",
        frame(),
        plan_report.prefix.display().to_string().dark_grey(),
        plan_report.virt_addr.display().to_string().bold()
    );

    if let Some(phy_addr) = &plan_report.phy_addr {
        println!(
            "{}  ↪ {}/{}:",
            frame(),
            plan_report.prefix.display().to_string().dark_grey(),
            phy_addr.display().to_string().underline_dark_grey()
        );
    }
}

pub fn print_plan(plan_report: &PlanReport) {
    // let prefix = plan_report.prefix.to_string_lossy().to_string().dark_grey();
    // let virt_addr = plan_report.virt_addr.to_string_lossy().to_string().bold();
    // let phy_addr = plan_report
    //     .phy_addr
    //     .as_ref()
    //     .map(|a| a.to_string_lossy().to_string().underline_dark_grey());

    // println!("║ At {}/{}:", prefix, virt_addr);

    // if let Some(phy_addr) = phy_addr {
    //     println!("║  ↪ {}/{}:", prefix, phy_addr);
    // }

    print_plan_addr(plan_report);

    for connector_op in &plan_report.connector_ops {
        let friendly_message = connector_op
            .friendly_message
            .clone()
            .unwrap_or(connector_op.op_definition.clone());

        println!("{}  ⟣ {}", frame(), colour_op_message(&friendly_message));
    }
    // println!("");
}
