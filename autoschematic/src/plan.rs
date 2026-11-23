use autoschematic_core::{config::AutoschematicConfig, report::PlanReport, util::load_autoschematic_config};
use crossterm::style::Stylize;

use crate::{
    CONNECTOR_CACHE, output,
    selection::select_paths,
    spinner::show_spinner,
    util::{colour_op_message, try_colour_op_message_diff},
};

pub async fn plan(
    prefix_filter: &Option<String>,
    connector_filter: &Option<String>,
    subpath_filter: &Option<String>,
) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let mut staged = crate::staged::StagedChanges::read(git2::Repository::discover(".")?)?;
    let staged_files = staged.paths();
    let no_staged_files = staged_files.is_empty();
    let staged_files = select_paths(
        &config,
        staged_files,
        prefix_filter.as_deref(),
        connector_filter.as_deref(),
        subpath_filter.as_deref(),
    )?;

    if staged_files.is_empty() {
        println!(
            "{}",
            if no_staged_files {
                "No files staged in git. Stage modified files with git add to plan or apply them."
            } else {
                "No staged files match the requested filters."
            }
        );
        return Ok(());
    }

    let batch = plan_batch(&config, &mut staged, staged_files, connector_filter).await?;
    if !batch.deferred.is_empty() {
        anyhow::bail!("Incomplete plan: {} resource(s) need outputs", batch.deferred.len());
    }
    Ok(())
}

#[derive(Default)]
pub struct PlanBatch {
    pub ready: Vec<PlanReport>,
    pub deferred: Vec<std::path::PathBuf>,
}

pub async fn plan_batch(
    config: &AutoschematicConfig,
    staged: &mut crate::staged::StagedChanges,
    paths: Vec<std::path::PathBuf>,
    connector: &Option<String>,
) -> anyhow::Result<PlanBatch> {
    let mut batch = PlanBatch::default();
    for path in paths {
        let spinner_stop = show_spinner().await;
        let report = autoschematic_core::workflow::plan::plan_staged(
            config,
            CONNECTOR_CACHE.clone(),
            None,
            connector,
            &path,
            staged.contents(&path)?,
        )
        .await?;
        let _ = spinner_stop.send(());
        let Some(report) = report else { continue };

        if !report.missing_outputs.is_empty() {
            print_plan_addr(&report);
            println!("{}  {}Missing outputs:", frame(), output::choose("⊬ ", ""));
            for output in &report.missing_outputs {
                println!("{}    {}[{}]", frame(), output.addr.display(), output.key);
            }
            batch.deferred.push(path);
        } else if !report.connector_ops.is_empty() {
            if batch.ready.is_empty() {
                print_frame_start();
            }
            print_plan(&report);
            batch.ready.push(report);
        }
    }
    if !batch.ready.is_empty() {
        print_frame_end();
    }
    if batch.deferred.is_empty() {
        println!("{}Plan complete.", output::choose(" ◇ ", ""));
        if batch.ready.is_empty() {
            println!("No changes for the selected staged resources.");
        }
    }
    Ok(batch)
}

pub fn print_frame_start() {
    if output::plain() {
        return;
    }
    let term_width = crossterm::terminal::size().unwrap_or((80, 0)).0;

    let frame_width = 80.min(term_width);

    let mut frame = String::new();

    frame.push_str(&"╔".dark_grey().to_string());

    for _ in 0..frame_width.saturating_sub(2) {
        frame.push_str(&"═".dark_grey().to_string());
    }

    frame.push_str(&"╗".dark_grey().to_string());

    println!("{}", frame);
}

pub fn frame() -> String {
    if output::plain() {
        String::new()
    } else {
        "║".dark_grey().to_string()
    }
}

pub fn print_frame_end() {
    if output::plain() {
        return;
    }
    let term_width = crossterm::terminal::size().unwrap_or((80, 0)).0;

    let frame_width = 80.min(term_width);

    let mut frame = String::new();

    frame.push_str(&"╚".dark_grey().to_string());

    for _ in 0..frame_width.saturating_sub(2) {
        frame.push_str(&"═".dark_grey().to_string());
    }

    frame.push_str(&"╝".dark_grey().to_string());

    println!("{}", frame);
}

pub fn print_plan_addr(plan_report: &PlanReport) {
    println!(
        "{} At {}/{}:",
        frame(),
        output::styled(plan_report.prefix.display().to_string().dark_grey()),
        output::styled(plan_report.virt_addr.display().to_string().bold())
    );

    if let Some(phy_addr) = &plan_report.phy_addr {
        println!(
            "{}  {} {}/{}:",
            frame(),
            output::choose("↪", "->"),
            output::styled(plan_report.prefix.display().to_string().dark_grey()),
            output::styled(phy_addr.display().to_string().underline_dark_grey())
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

        let coloured_message = try_colour_op_message_diff(&friendly_message).unwrap_or(colour_op_message(&friendly_message));

        for (i, line) in coloured_message.lines().enumerate() {
            if i == 0 {
                println!("{}  {} {}", frame(), output::choose("⟣", "-"), line)
            } else {
                println!("{}  {}", frame(), line)
            }
        }
    }
}
