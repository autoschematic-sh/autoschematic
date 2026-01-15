use std::{
    collections::HashSet,
    io::Write,
    process::{Command, Stdio},
};

use crossterm::style::Stylize;
use dialoguer::Confirm;
use rand::Rng;

use autoschematic_core::{
    git_util::{get_staged_files, git_add},
    report::{ApplyReport, PlanReport, PlanReportSet},
    template::ReadOutput,
    util::{load_autoschematic_config, repo_root},
};

use crate::{
    CONNECTOR_CACHE,
    plan::{frame, print_frame_end, print_frame_start, print_plan, print_plan_addr},
    safety_lock::check_safety_lock,
    spinner::show_spinner,
    util::{colour_op_message, try_colour_op_message_diff},
};

pub async fn apply(
    _prefix_filter: Option<String>,
    connector_filter: Option<String>,
    _subpath_filter: Option<String>,
    ask_confirm: bool,
    skip_commit: bool,
) -> anyhow::Result<Vec<ApplyReport>> {
    check_safety_lock()?;

    let repo_root = repo_root()?;
    let config = load_autoschematic_config()?;

    let staged_files = get_staged_files()?;

    let keystore = None;

    let mut wrote_files = false;

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

    let mut plan_report_set = PlanReportSet::default();
    let mut apply_report_set = Vec::new();

    let mut need_print_frame_start = true;
    let mut need_print_frame_end = false;

    if staged_files.is_empty() {
        println!(" ∅  No files staged in git. Stage modified files with git add to plan or apply them.");
        return Ok(apply_report_set);
    }

    // let mut unbundle_results = Vec::new();
    // for path in &staged_files {
    //     if let Some(unbundle_report) = autoschematic_core::workflow::unbundle::unbundle(
    //         &config,
    //         connector_cache.clone(),
    //         keystore.clone(),
    //         &connector,
    //         &path,
    //     )
    //     .await?
    //     {
    //         if let Some(elements) = unbundle_report.elements {
    //             for element in elements {
    //                 unbundle_results.push(element.addr);
    //             }
    //         }
    //     }
    // }
    //
    let mut deferred: Vec<PlanReport> = Vec::new();
    let mut set_outputs: HashSet<ReadOutput> = HashSet::new();

    for path in staged_files {
        let spinner_stop = show_spinner().await;

        //     // TODO track if no staged files matched FilterResponse::Resource...
        //     // autoschematic_core::workflow::filter::filter(&config, &connector_cache, keystore, prefix, addr)

        let Some(plan_report) = autoschematic_core::workflow::plan::plan(
            &config,
            CONNECTOR_CACHE.clone(),
            keystore.clone(),
            &connector_filter,
            &path,
        )
        .await?
        else {
            spinner_stop.send(()).unwrap();
            continue;
        };

        spinner_stop.send(()).unwrap();

        if !plan_report.missing_outputs.is_empty() {
            deferred.push(plan_report.clone());
        }

        if plan_report.connector_ops.is_empty() {
            continue;
        }

        if need_print_frame_start {
            need_print_frame_start = false;
            need_print_frame_end = true;
            print_frame_start();
        }

        print_plan(&plan_report);

        plan_report_set.plan_reports.push(plan_report);
    }

    if need_print_frame_end {
        print_frame_end();
    }

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
        return Ok(apply_report_set);
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
        let Some(apply_report) = autoschematic_core::workflow::apply::apply(
            &config,
            CONNECTOR_CACHE.clone(),
            keystore.clone(),
            &connector_filter,
            &plan_report,
        )
        .await?
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

        for output in &apply_report.outputs {
            if let Some(ref friendly_message) = output.friendly_message {
                let coloured_message =
                    try_colour_op_message_diff(friendly_message).unwrap_or(colour_op_message(friendly_message));

                for (i, line) in coloured_message.lines().enumerate() {
                    if i == 0 {
                        // println!("{}  ⟣ {}", frame(), line)
                        println!("{}  ⟖ {}", frame(), line);
                    } else {
                        println!("{}  {}", frame(), line)
                    }
                }
            }

            if let Some(ref report_outputs) = output.outputs {
                for (key, _) in report_outputs {
                    set_outputs.insert(ReadOutput {
                        addr: plan_report.virt_addr.clone(),
                        key: key.clone(),
                    });
                }
            }
        }

        for path in &apply_report.wrote_files {
            git_add(&repo_root, path)?;
        }

        wrote_files = true;

        apply_report_set.push(apply_report);
    }
    if need_print_frame_end {
        print_frame_end();
    }

    let mut did_make_output_progress = false;

    if !deferred.is_empty() {
        println!(" ⊬ Some files were not applied as they were missing outputs.");
        for plan_report in deferred {
            print_plan_addr(&plan_report);
            for output in &plan_report.missing_outputs {
                if set_outputs.contains(output) {
                    did_make_output_progress = true;
                    println!(" {} {}[{}]  ", "[SET!]".green(), output.addr.display(), output.key);
                } else {
                    println!("        {}[{}]", output.addr.display(), output.key);
                }
            }
        }

        if did_make_output_progress {
            let do_reapply = Confirm::new()
                .with_prompt(" ◈ Apply succeeded! Some resources were deferred on outputs that are now available. Do you wish to continue applying?")
                .default(true)
                .interact()
                .unwrap();

            if do_reapply {
                Box::pin(apply(
                    _prefix_filter,
                    connector_filter,
                    _subpath_filter,
                    ask_confirm,
                    skip_commit,
                ))
                .await?;
            }
        } else {
            let do_reapply = Confirm::new()
                .with_prompt(
                    " ◈ Apply succeeded, but some resources are still deferred. Do you wish to continue applying anyway?",
                )
                .default(true)
                .interact()
                .unwrap();

            if do_reapply {
                // apply(_prefix_filter, connector_filter, _subpath_filter, ask_confirm, skip_commit).await?;
                Box::pin(apply(
                    _prefix_filter,
                    connector_filter,
                    _subpath_filter,
                    ask_confirm,
                    skip_commit,
                ))
                .await?;
            }
        }
    } else if wrote_files && !skip_commit {
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

    Ok(apply_report_set)
}
