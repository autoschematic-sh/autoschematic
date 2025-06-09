use std::time::Duration;

use crate::task::util::{create_comment, wait_for_comment_types};

use super::PythonTask;

impl PythonTask {
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
            &format!(
                "autoschematic pull-state {} {}",
                prefix_filter, connector_filter
            ),
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

    pub async fn plan(
        &mut self,
        issue_number: u64,
    ) -> anyhow::Result<String> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        create_comment(
            &mut self.outbox,
            &self.owner,
            &self.repo,
            issue_number,
            &format!("autoschematic plan -p {}", self.prefix.to_string_lossy()),
        )
        .await?;

        let (comment_type, _comment) = wait_for_comment_types(
            &self.owner,
            &self.repo,
            issue_number,
            &[
                "plan_overall_success",
                "plan_overall_success_with_deferrals",
                "plan_no_changes",
                "plan_error",
                "plan_overall_error",
                "filter_matched_no_files",
                "misc_error",
            ],
            &mut self.inbox,
        )
        .await?;
        Ok(comment_type)
    }

    pub async fn apply(
        &mut self,
        issue_number: u64,
    ) -> anyhow::Result<String> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        create_comment(
            &mut self.outbox,
            &self.owner,
            &self.repo,
            issue_number,
            "autoschematic apply",
        )
        .await?;

        let (comment_type, _comment) = wait_for_comment_types(
            &self.owner,
            &self.repo,
            issue_number,
            &["apply_overall_success", "apply_success", "apply_error"],
            &mut self.inbox,
        )
        .await?;
        Ok(comment_type)
    }
}
