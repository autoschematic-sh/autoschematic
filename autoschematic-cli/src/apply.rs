use std::{
    io::Write,
    process::{Command, Stdio},
};

use anyhow::{Context, bail};
use dialoguer::Confirm;
use rand::Rng;
use ron::ser::PrettyConfig;

use autoschematic_core::{
    config::AutoschematicConfig,
    connector_cache::{self, ConnectorCache},
    git_util::{get_staged_files, git_add, git_commit},
    report::PlanReportSet,
    util::{RON, repo_root},
};

use crate::{config::load_autoschematic_config, ui};

pub async fn apply(prefix: Option<String>, connector: Option<String>, subpath: Option<String>) -> anyhow::Result<()> {
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
            .output()
            .expect("Git: pre-commit hooks failed!");
    }

    let mut plan_report_set = PlanReportSet::default();
    for path in staged_files {
        if let Some(plan_report) =
            autoschematic_core::workflow::plan::plan(&config, &connector_cache, keystore, &connector, &path).await?
        {
            println!("-----------------------------");
            println!("{}", path.display());
            println!("-----------------------------");
            println!(
                "{}",
                RON.to_string_pretty(
                    &plan_report
                        .connector_ops
                        .iter()
                        .map(|op| op.friendly_message.clone().unwrap_or(op.op_definition.clone()))
                        .collect::<Vec<String>>(),
                    PrettyConfig::default()
                )
                .context("Formatting plan report")?
            );

            plan_report_set.plan_reports.push(plan_report);
        }
    }

    const CHARSET: &[u8] = b"1234567890";
    const PASSWORD_LEN: usize = 4;
    let mut rng = rand::rng();

    let verify_code: String = (0..PASSWORD_LEN)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    println!("Plan complete.");
    println!("Type {} to execute all of the above actions and commit.", verify_code);
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
                continue;
            }
        }
    }

    for plan_report in plan_report_set.plan_reports {
        if let Some(apply_report) =
            autoschematic_core::workflow::apply::apply(&config, &connector_cache, keystore, &connector, &plan_report).await?
        {
            println!("-----------------------------");
            println!("{}", plan_report.prefix.join(plan_report.virt_addr).display());
            println!("-----------------------------");

            for output in apply_report.outputs {
                if let Some(friendly_message) = output.friendly_message {
                    println!("{}", friendly_message);
                }
            }

            for path in &apply_report.wrote_files {
                git_add(&repo_root, path)?;
            }
            wrote_files = true;
        }
    }

    if wrote_files {
        let do_commit = Confirm::new()
            .with_prompt("Apply succeeded! Do you wish to run git commit to track the new state?")
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
