use std::{
    io::Write,
    process::{Command, Stdio},
};

use anyhow::Context;
use colored::Colorize;
use crossterm::style::Stylize;
use dialoguer::Confirm;
use rand::Rng;
use ron::ser::PrettyConfig;
use ron_pfnsec_fork as ron;

use autoschematic_core::{
    connector_cache::ConnectorCache,
    git_util::{get_staged_files, git_add},
    report::PlanReportSet,
    util::{RON, repo_root},
};

use crate::{
    config::load_autoschematic_config,
    plan::{print_frame_end, print_frame_start, print_plan, print_plan_addr},
    spinner::spinner::show_spinner,
};

pub async fn apply(
    prefix: Option<String>,
    connector: Option<String>,
    subpath: Option<String>,
    ask_confirm: bool,
    skip_commit: bool,
) -> anyhow::Result<()> {
    let repo_root = repo_root()?;
    let config = load_autoschematic_config()?;

    let staged_files = get_staged_files()?;

    let connector_cache = ConnectorCache::default();

    let keystore = None;

    let mut wrote_files = false;

    let pre_commit_hook = repo_root.join(".git").join("pre-commit");

    if pre_commit_hook.is_file() {
        Command::new("sh")
            .arg(pre_commit_hook)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .output()
            .expect("Git: pre-commit hooks failed!");
    }

    let mut plan_report_set = PlanReportSet::default();

    let mut need_print_frame_start = true;

    if staged_files.is_empty() {
        println!(" ∅  No files staged in git. Stage modified files with git add to plan or apply them.");
        return Ok(());
    }

    for path in staged_files {
        let spinner_stop = show_spinner().await;

        //     // TODO track if no staged files matched FilterOutput::Resource...
        //     // autoschematic_core::workflow::filter::filter(&config, &connector_cache, keystore, prefix, addr)

        let Some(plan_report) =
            autoschematic_core::workflow::plan::plan(&config, &connector_cache, keystore, &connector, &path).await?
        else {
            spinner_stop.send(()).unwrap();
            continue;
        };

        spinner_stop.send(()).unwrap();

        if plan_report.connector_ops.len() == 0 {
            continue;
        }

        if need_print_frame_start {
            need_print_frame_start = false;
            print_frame_start();
        }

        print_plan(&plan_report);

        plan_report_set.plan_reports.push(plan_report);
    }

    print_frame_end();

    const CHARSET: &[u8] = b"1234567890";
    const PASSWORD_LEN: usize = 4;

    let verify_code: String = {
        let mut rng = rand::rng();
        (0..PASSWORD_LEN)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    };

    println!(" ◇ Plan complete.");
    if plan_report_set.plan_reports.is_empty() {
        println!(
            " ≡ All plans are empty, implying that the remote configuration matches the desired configuration for all staged files."
        );
        return Ok(());
    }

    if ask_confirm {
        println!(
            "Type {} to {} of the above actions and commit.",
            verify_code.clone().bold(),
            "execute all".underline_dark_grey()
        );
        println!("Hit Ctrl-c to cancel.");

        loop {
            print!(">");
            let _ = std::io::stdout().flush();

            let mut input_line = String::new();
            std::io::stdin().read_line(&mut input_line).expect("Failed to read line");
            let verify_code_attempt = input_line.trim();
            match verify_code_attempt {
                s if s == verify_code => {
                    break;
                }
                _ => {
                    println!("Wrong code.");
                    continue;
                }
            }
        }
    }

    let mut need_print_frame_start = true;
    let mut need_print_frame_end = false;
    for plan_report in plan_report_set.plan_reports {
        let spinner_stop = show_spinner().await;
        let Some(apply_report) =
            autoschematic_core::workflow::apply::apply(&config, &connector_cache, keystore, &connector, &plan_report).await?
        else {
            spinner_stop.send(()).unwrap();
            continue;
        };

        spinner_stop.send(()).unwrap();

        if need_print_frame_start {
            need_print_frame_start = false;
            need_print_frame_end = true;
            print_frame_start();
        }

        print_plan_addr(&plan_report);

        for output in apply_report.outputs {
            if let Some(friendly_message) = output.friendly_message {
                println!("║  ⟖ {}", friendly_message);
            }
        }

        for path in &apply_report.wrote_files {
            git_add(&repo_root, path)?;
        }
        wrote_files = true;

        if need_print_frame_end {
            print_frame_end();
        }
    }

    if wrote_files && !skip_commit {
        let do_commit = Confirm::new()
            .with_prompt(" ◈ Apply succeeded! Do you wish to run git commit to track the new state?")
            .default(true)
            .interact()
            .unwrap();

        if do_commit {
            Command::new("git")
                .arg("commit")
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .output()
                .expect("git commit: failed to execute");
        }
    }

    Ok(())
}
