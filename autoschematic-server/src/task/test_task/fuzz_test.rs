use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};

use crate::{
    RON,
    github_util::{create_pull_request, merge_pr},
    task::util::wait_for_comment_types,
};

use autoschematic_core::{
    git_util::{checkout_branch, checkout_new_branch, get_head_sha, git_add, git_commit_and_push, pull_with_rebase},
    util::copy_dir_all,
};

use super::TestTask;

#[derive(Clone, Serialize, Deserialize)]
struct FuzzConfig {
    states: Vec<String>,
    connector_filter: Option<String>,
}

impl TestTask {
    pub async fn run_fuzz_test(&mut self, path: &Path) -> anyhow::Result<()> {
        tracing::warn!("FUZZ test! Path = {:?}, curdir = {:?}", path, std::env::current_dir());

        let repo_path = self.repo_path();
        let fuzz_config: FuzzConfig = RON.from_str(&std::fs::read_to_string(path.join("fuzz_config.ron"))?)?;

        // let mut rng = rand::thread_rng();
        // fuzz_config.states.shuffle(&mut rng);
        let fuzz_config = fuzz_config.clone();

        let connector_filter = fuzz_config.connector_filter.map(|c| format!("-c {}", c)).unwrap_or_default();
        let prefix_filter = format!("-p {}", self.prefix.to_string_lossy());

        loop {
            let rand_suffix: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

            let branch_name = format!("test-fuzz/{}-{}", path.to_str().unwrap_or_default(), rand_suffix);

            checkout_new_branch(&repo_path, &branch_name)
                .await
                .context("Checking out branch")?;

            git_commit_and_push(
                &repo_path,
                &branch_name,
                &self.token,
                &format!("Create branch {}", branch_name),
            )
            .context("git_commit_and_push")?;

            let issue_number = create_pull_request(&self.owner, &self.repo, &branch_name, &branch_name, "main", &self.client)
                .await
                .context("Creating pull request")?;

            wait_for_comment_types(
                &self.owner.clone(),
                &self.repo.clone(),
                issue_number,
                &["greeting"],
                &mut self.inbox,
            )
            .await?;

            tracing::error!("Got the greeting message!");
            for state in &fuzz_config.states {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let comment_type = self.pull_state(issue_number, &prefix_filter, &connector_filter).await?;

                match comment_type.as_str() {
                    "pull_state_success" => {}
                    "pull_state_clean" => {}
                    "pull_state_with_deferrals" => {}
                    "filter_matched_no_files" => {}
                    "pull_state_error" => {
                        bail!("Pull state threw an error. Quitting!")
                    }
                    "misc_error" => {
                        bail!("Pull state threw an error. Quitting!")
                    }
                    t => {
                        bail!("unexpected message type {}", t)
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
                pull_with_rebase(&repo_path, &branch_name, &self.token)?;

                // // clear_prefix(&self.prefix).await?;
                // clear_prefix_keep_outputs(&self.prefix).await?;

                // git_add(&repo_path, &PathBuf::from("."))?;
                // tokio::time::sleep(Duration::from_secs(1)).await;
                // git_commit_and_push(
                //     &repo_path,
                //     &branch_name,
                //     &self.token,
                //     &format!("Clear state"),
                // )?;

                // self.plan_apply_import_complete(
                //     issue_number,
                //     &prefix_filter,
                //     &connector_filter,
                //     &repo_path,
                //     &branch_name,
                // ).await?;

                // // clear_prefix(&self.prefix).await?;
                // clear_prefix_keep_outputs(&self.prefix).await?;
                copy_dir_all(path.join(state), &self.prefix).context("copy dir all")?;

                tokio::time::sleep(Duration::from_secs(1)).await;
                git_add(&repo_path, &PathBuf::from("."))?;
                git_commit_and_push(&repo_path, &branch_name, &self.token, &format!("Fuzz state {}", state))?;

                self.plan_apply_pull_state_complete(issue_number, &prefix_filter, &connector_filter, &repo_path, &branch_name)
                    .await?;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            let sha = get_head_sha(&repo_path)?;
            merge_pr(&self.client, &self.owner, &self.repo, issue_number, &sha).await?;

            checkout_branch(&repo_path, "main").await?;

            tokio::time::sleep(Duration::from_secs(1)).await;
            pull_with_rebase(&repo_path, "main", &self.token)?;
        }
    }
}
