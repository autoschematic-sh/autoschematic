use std::{path::Path, time::Duration};

use anyhow::bail;

use crate::task::util::{create_comment, wait_for_comment_types};
use autoschematic_core::git_util::pull_with_rebase;

use super::TestTask;

impl TestTask {
    pub async fn pull_state(
        &mut self,
        issue_number: u64,
        prefix_filter: &str,
        connector_filter: &str,
    ) -> anyhow::Result<String> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        create_comment(
            &mut self.outbox,
            &self.owner,
            &self.repo,
            issue_number,
            &format!("autoschematic pull-state {} {}", prefix_filter, connector_filter),
        )
        .await?;

        let (comment_type, comment) = wait_for_comment_types(
            &self.owner,
            &self.repo,
            issue_number,
            &[
                "pull_state_success",
                "pull_state_with_deferrals",
                "pull_state_clean",
                "pull_state_error",
                "filter_matched_no_files",
                "misc_error",
            ],
            &mut self.inbox,
        )
        .await?;
        Ok(comment_type)
    }

    pub async fn plan(&mut self, issue_number: u64, prefix_filter: &str, connector_filter: &str) -> anyhow::Result<String> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        create_comment(
            &mut self.outbox,
            &self.owner,
            &self.repo,
            issue_number,
            &format!("autoschematic plan {} {}", prefix_filter, connector_filter),
        )
        .await?;

        let (comment_type, comment) = wait_for_comment_types(
            &self.owner,
            &self.repo,
            issue_number,
            &[
                "plan_overall_success",
                "plan_overall_success_with_deferrals",
                "plan_overall_error",
                "plan_no_changes",
                "filter_matched_no_files",
                "misc_error",
            ],
            &mut self.inbox,
        )
        .await?;
        Ok(comment_type)
    }

    pub async fn apply(&mut self, issue_number: u64, prefix_filter: &str, connector_filter: &str) -> anyhow::Result<String> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        create_comment(&mut self.outbox, &self.owner, &self.repo, issue_number, "autoschematic apply").await?;

        let (comment_type, comment) = wait_for_comment_types(
            &self.owner,
            &self.repo,
            issue_number,
            &["apply_overall_success", "apply_error", "misc_error"],
            &mut self.inbox,
        )
        .await?;
        Ok(comment_type)
    }

    pub async fn import(&mut self, issue_number: u64, prefix_filter: &str, connector_filter: &str) -> anyhow::Result<String> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        create_comment(
            &mut self.outbox,
            &self.owner,
            &self.repo,
            issue_number,
            &format!("autoschematic import {} {}", prefix_filter, connector_filter),
        )
        .await?;

        let (comment_type, comment) = wait_for_comment_types(
            &self.owner,
            &self.repo,
            issue_number,
            &["import_success", "import_error", "misc_error"],
            &mut self.inbox,
        )
        .await?;
        Ok(comment_type)
    }

    pub async fn import_overwrite(
        &mut self,
        issue_number: u64,
        prefix_filter: &str,
        connector_filter: &str,
    ) -> anyhow::Result<String> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        create_comment(
            &mut self.outbox,
            &self.owner,
            &self.repo,
            issue_number,
            &format!("autoschematic import --overwrite {} {}", prefix_filter, connector_filter),
        )
        .await?;

        let (comment_type, comment) = wait_for_comment_types(
            &self.owner,
            &self.repo,
            issue_number,
            &["import_success", "import_error", "misc_error"],
            &mut self.inbox,
        )
        .await?;
        Ok(comment_type)
    }

    pub async fn plan_apply_import_complete(
        &mut self,
        issue_number: u64,
        prefix_filter: &str,
        connector_filter: &str,
        repo_path: &Path,
        branch_name: &str,
    ) -> anyhow::Result<()> {
        'reapply: loop {
            let comment_type = self.plan(issue_number, prefix_filter, connector_filter).await?;

            let mut do_apply = false;
            let mut have_deferrals = false;
            match comment_type.as_str() {
                "plan_overall_success" => {
                    do_apply = true;
                }
                "plan_overall_success_with_deferrals" => {
                    have_deferrals = true;
                    do_apply = true;
                }
                "plan_no_changes" => {}
                "filter_matched_no_files" => {}
                "plan_overall_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                "plan_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                "misc_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                t => {
                    bail!("unexpected message type {}", t)
                }
            }

            if do_apply {
                let comment_type = self.apply(issue_number, prefix_filter, connector_filter).await?;

                match comment_type.as_str() {
                    "apply_overall_success" => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        pull_with_rebase(repo_path, branch_name, &self.token)?;
                    }
                    "apply_success" => {}
                    "apply_error" => {
                        bail!("Apply threw an error. Quitting!")
                    }
                    "misc_error" => {
                        bail!("Apply threw an error. Quitting!")
                    }
                    t => {
                        bail!("unexpected message type {}", t)
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
                let comment_type = self.import_overwrite(issue_number, prefix_filter, connector_filter).await?;

                match comment_type.as_str() {
                    "import_success" => {}
                    "import_error" => {
                        bail!("Import threw an error. Quitting!")
                    }
                    "misc_error" => {
                        bail!("Import threw an error. Quitting!")
                    }
                    t => {
                        bail!("unexpected message type {}", t)
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
                pull_with_rebase(repo_path, branch_name, &self.token)?;

                if have_deferrals {
                    continue;
                } else {
                    break;
                }
            } else {
                break 'reapply;
            }
        }
        Ok(())
    }

    pub async fn plan_apply_pull_state_complete(
        &mut self,
        issue_number: u64,
        prefix_filter: &str,
        connector_filter: &str,
        repo_path: &Path,
        branch_name: &str,
    ) -> anyhow::Result<()> {
        'reapply: loop {
            let comment_type = self.plan(issue_number, prefix_filter, connector_filter).await?;

            let mut do_apply = false;
            let mut have_deferrals = false;
            match comment_type.as_str() {
                "plan_overall_success" => {
                    do_apply = true;
                }
                "plan_overall_success_with_deferrals" => {
                    have_deferrals = true;
                    do_apply = true;
                }
                "plan_no_changes" => {}
                "filter_matched_no_files" => {}
                "plan_overall_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                "plan_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                "misc_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                t => {
                    bail!("unexpected message type {}", t)
                }
            }

            if do_apply {
                let comment_type = self.apply(issue_number, prefix_filter, connector_filter).await?;

                match comment_type.as_str() {
                    "apply_overall_success" => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        pull_with_rebase(repo_path, branch_name, &self.token)?;
                    }
                    "apply_success" => {}
                    "apply_error" => {
                        bail!("Apply threw an error. Quitting!")
                    }
                    "misc_error" => {
                        bail!("Apply threw an error. Quitting!")
                    }
                    t => {
                        bail!("unexpected message type {}", t)
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
                let comment_type = self.pull_state(issue_number, prefix_filter, connector_filter).await?;
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
                pull_with_rebase(repo_path, branch_name, &self.token)?;

                if have_deferrals {
                    continue;
                } else {
                    break;
                }
            } else {
                break 'reapply;
            }
        }
        Ok(())
    }
}
