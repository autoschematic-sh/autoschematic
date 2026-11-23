use std::{
    io::{BufRead, Write},
    process::{Command, Stdio},
};

use crossterm::style::Stylize;
use dialoguer::Confirm;
use rand::Rng;

use autoschematic_core::{
    git_util::git_add,
    report::ApplyReport,
    util::{load_autoschematic_config, repo_root},
};

use crate::{
    CONNECTOR_CACHE, output,
    plan::{PlanBatch, frame, plan_batch, print_frame_end, print_frame_start, print_plan_addr},
    safety_lock::check_safety_lock,
    selection::select_paths,
    spinner::show_spinner,
    util::{colour_op_message, try_colour_op_message_diff},
};

pub async fn apply(
    prefix_filter: Option<String>,
    connector_filter: Option<String>,
    subpath_filter: Option<String>,
    ask_confirm: bool,
    skip_commit: bool,
) -> anyhow::Result<Vec<ApplyReport>> {
    check_safety_lock()?;

    let repo_root = repo_root()?;
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
        return Ok(Vec::new());
    }

    let keystore = None;

    let pre_commit_hook = repo_root.join(".git").join("pre-commit");

    if pre_commit_hook.is_file() {
        Command::new("sh")
            .arg(pre_commit_hook)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit())
            .output()
            .expect("Git: pre-commit hooks failed!");
    }

    let mut apply_report_set = Vec::new();
    let mut pending = staged_files;
    while !pending.is_empty() {
        let batch = plan_batch(&config, &mut staged, pending, &connector_filter).await?;
        check_deferred(&batch, ask_confirm, apply_report_set.len())?;
        if batch.ready.is_empty() {
            break;
        }
        if ask_confirm {
            confirm_operations()?;
        }
        check_safety_lock()?;
        pending = batch.deferred;
        let mut need_print_frame_start = true;
        let mut need_print_frame_end = false;
        for plan_report in batch.ready {
            let spinner_stop = show_spinner().await;
            let Some(apply_report) = autoschematic_core::workflow::apply::apply(
                &config,
                CONNECTOR_CACHE.clone(),
                keystore.clone(),
                &plan_report.connector_def.as_ref().map(|c| c.shortname.clone()),
                &plan_report,
            )
            .await?
            else {
                spinner_stop.send(()).unwrap();
                anyhow::bail!(
                    "No connector applied {}",
                    plan_report.prefix.join(&plan_report.virt_addr).display()
                );
            };

            spinner_stop.send(()).unwrap();

            if need_print_frame_start {
                need_print_frame_start = false;
                need_print_frame_end = true;
                print_frame_start();
            }

            print_plan_addr(&plan_report);

            for output in &apply_report.outputs {
                if let Some(ref friendly_message) = output.friendly_message {
                    let coloured_message =
                        try_colour_op_message_diff(friendly_message).unwrap_or(colour_op_message(friendly_message));

                    for (i, line) in coloured_message.lines().enumerate() {
                        if i == 0 {
                            // println!("{}  ⟣ {}", frame(), line)
                            println!("{}  {} {}", frame(), output::choose("⟖", "-"), line);
                        } else {
                            println!("{}  {}", frame(), line)
                        }
                    }
                }
            }

            for path in &apply_report.wrote_files {
                git_add(&repo_root, path)?;
            }

            apply_report_set.push(apply_report);
        }
        if need_print_frame_end {
            print_frame_end();
        }
    }
    if !apply_report_set.is_empty() && !skip_commit {
        let do_commit = Confirm::new()
            .with_prompt("Apply succeeded! Do you wish to run git commit to track the new state?")
            .default(true)
            .interact()?;

        if do_commit {
            let status = Command::new("git")
                .arg("commit")
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .status()?;
            anyhow::ensure!(status.success(), "git commit failed: {status}");
        }
    }

    Ok(apply_report_set)
}

fn check_deferred(batch: &PlanBatch, interactive: bool, applied: usize) -> anyhow::Result<()> {
    if !batch.deferred.is_empty() && (!interactive || batch.ready.is_empty()) {
        anyhow::bail!(
            "Incomplete apply: {} resource(s) need outputs; {applied} resource(s) applied. Apply the prerequisites first.",
            batch.deferred.len()
        );
    }
    Ok(())
}

fn confirm_operations() -> anyhow::Result<()> {
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

    println!(
        "Type {} to {} of the above actions.",
        output::styled(verify_code.clone().bold()),
        output::styled("execute all".underline_dark_grey())
    );
    println!("Hit Ctrl-c to cancel.");

    loop {
        print!(">");
        let _ = std::io::stdout().flush();

        let input_line = read_confirmation(&mut std::io::stdin().lock())?;
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
    Ok(())
}

fn read_confirmation(input: &mut impl BufRead) -> anyhow::Result<String> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        anyhow::bail!("Confirmation input closed. No actions confirmed; use --skip-confirm for unattended apply.");
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_outputs_stop_unattended_apply_before_any_ready_operations() {
        let mut batch = PlanBatch::default();
        batch.deferred.push("prod/child".into());
        assert!(
            check_deferred(&batch, true, 1)
                .unwrap_err()
                .to_string()
                .contains("1 resource(s) applied")
        );
        batch.ready.push(Default::default());
        assert!(check_deferred(&batch, false, 0).is_err());
        assert!(check_deferred(&batch, true, 0).is_ok());
        batch.deferred.clear();
        assert!(check_deferred(&batch, false, 0).is_ok());
    }

    #[test]
    fn confirmation_eof_is_an_error() {
        assert!(read_confirmation(&mut std::io::Cursor::new("")).is_err());
        assert_eq!(read_confirmation(&mut std::io::Cursor::new("1234\n")).unwrap(), "1234\n");
    }
}
