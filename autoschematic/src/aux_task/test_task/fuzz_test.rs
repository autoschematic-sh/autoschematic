use std::path::{Path, PathBuf};

use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};

use autoschematic_core::{
    git_util::git_commit,
    util::{RON, diff_text_markdown, load_autoschematic_config},
    workflow::check_drift,
};
use tokio::process::Command;

use crate::{CONNECTOR_CACHE, apply, aux_task::test_task::TestTask, util::try_colour_op_message_diff};

#[derive(Clone, Serialize, Deserialize)]
struct FuzzConfig {
    states: Vec<String>,
    connector_filter: Option<String>,
}

impl TestTask {
    pub async fn run_fuzz_test(&self, path: &Path) -> anyhow::Result<()> {

        // let connector_filter = fuzz_config.connector_filter.map(|c| format!("-c {c}")).unwrap_or_default();
        // let prefix_filter = format!("-p {}", self.prefix.to_string_lossy());
        //
        let rand_suffix: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();
        // Pardon my laziness!
        let output = Command::new("git").arg("checkout").arg("main").output().await?;
        eprintln!("{}", str::from_utf8(&output.stderr).ok().unwrap_or_default());
        println!("{}", str::from_utf8(&output.stdout).ok().unwrap_or_default());


        let config = load_autoschematic_config()?;
        let fuzz_config: FuzzConfig = RON.from_str(&std::fs::read_to_string(path.join("fuzz_config.ron"))?)?;


        let output = Command::new("git")
            .arg("checkout")
            .arg("-b")
            .arg(format!("fuzz-test-{}", rand_suffix))
            .output()
            .await?;

        eprintln!("{}", str::from_utf8(&output.stderr).ok().unwrap_or_default());
        println!("{}", str::from_utf8(&output.stdout).ok().unwrap_or_default());
        // ...ahem...

        let mut i = 0;
        let mut samples: [u128; 10] = [0; 10];
        loop {
            let rand_suffix: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

            for state in &fuzz_config.states {
                println!("Fuzzing at state {}", state);
                if i == 10 {
                    let mut sum = 0;
                    for sample in samples {
                        sum += sample;
                    }

                    println!("Frequency: {}hz", 1.0 / ((sum / 10_000_000_000) as f64));
                    i = 0;
                }
                samples[i] = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
                i += 1;
                // copy_dir_all(path.join(state), &self.prefix).context("copy dir all")?;

                // // tokio::time::sleep(Duration::from_secs(1)).await;

                // git_add(&PathBuf::from("."), &PathBuf::from("."))?;

                // stage_diff_relative_to_target(&PathBuf::from("."), state)?;

                // git_commit_and_push(&repo_path, &branch_name, &self.token, &format!("Fuzz state {}", state))?;
                //

                // If we have a snapshot of state in a branch called "world-state-42",
                // we want to stage changes in the current branch such that committing
                // would set all desired state to match that in the snapshot branch.
                // Likewise, if the connectors under test are good, applying
                // should also set the current state to match what's in this snapshot branch!
                // So, to test this assumption, we run check_drift after this whole song-and-dance.
                let output = Command::new("git")
                    .arg("restore")
                    .arg(format!("--source={}", state))
                    .arg("--staged")
                    .arg("--worktree")
                    .arg("--")
                    .arg(".")
                    .output()
                    .await?;

                eprintln!("{}", str::from_utf8(&output.stderr).ok().unwrap_or_default());
                println!("{}", str::from_utf8(&output.stdout).ok().unwrap_or_default());

                let prefix = None;
                let connector = None;
                let subpath = None;
                let ask_confirm = false;
                let skip_commit = true;

                let apply_reports = apply::apply(prefix, connector, subpath, ask_confirm, skip_commit).await?;

                let message = format!("fuzz-test-{rand_suffix}");
                println!("{}", message);
                git_commit(&PathBuf::from("."), "autoschematic-fuzz", "fuzz@autoschematic.sh", &message).unwrap();

                for apply_report in apply_reports {
                    println!(
                        "Checking drift at: {}/{}",
                        apply_report.prefix.display(),
                        apply_report.virt_addr.display()
                    );

                    match check_drift::check_drift(
                        &config,
                        &CONNECTOR_CACHE,
                        None,
                        &apply_report.prefix,
                        &apply_report.virt_addr,
                    )
                    .await?
                    {
                        check_drift::CheckDriftResult::NeitherExist => {
                            println!("check_drift: Neither exist?");
                        }
                        check_drift::CheckDriftResult::InvalidAddress => {
                            println!("check_drift: invalid address?");
                        }
                        check_drift::CheckDriftResult::NotEqual { current, desired } => {
                            println!("check_drift: not equal!");
                            match (current, desired) {
                                (None, None) => {}
                                (None, Some(_)) => {}
                                (Some(_), None) => {}
                                (Some(current), Some(desired)) => {
                                    let diff = diff_text_markdown(str::from_utf8(&current)?, str::from_utf8(&desired)?)?;
                                    println!("{}", try_colour_op_message_diff(&diff).unwrap_or(diff))
                                }
                            }
                        }
                        check_drift::CheckDriftResult::Equal => {
                            println!("check_drift: equal!");
                        }
                    }
                }
            }
        }
    }
}
