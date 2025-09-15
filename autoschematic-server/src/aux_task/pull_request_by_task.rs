use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use async_trait::async_trait;
use octocrab::{Octocrab, models::InstallationId};
use regex::Regex;
use secrecy::SecretBox;
use tempdir::TempDir;

// mod fuzz_test;
// mod util;

use crate::{
    chwd::ChangeWorkingDirectory,
    credentials,
    error::{AutoschematicServerError, AutoschematicServerErrorType},
};

use autoschematic_core::{
    aux_task::{Task, TaskInbox, TaskOutbox, message::TaskMessage, state::TaskState, util::drain_inbox},
    git_util::clone_repo,
};

pub enum TestType {
    Fuzz(PathBuf),
}

pub struct PullRequestByTask {
    owner: String,
    repo: String,
    prefix: PathBuf,
    task_addr: PathBuf,

    temp_dir: TempDir,
    token: SecretBox<str>,
    client: Octocrab,
    inbox: TaskInbox,
    outbox: TaskOutbox,
}

#[async_trait]
impl Task for PullRequestByTask {
    async fn new(
        owner: &str,
        repo: &str,
        prefix: &Path,
        name: &str,
        inbox: TaskInbox,
        outbox: TaskOutbox,
        installation_id: u64,
    ) -> Result<Box<dyn Task>, anyhow::Error>
    where
        Self: Sized,
    {
        let (client, token) = credentials::octocrab_installation_client(InstallationId(installation_id)).await?;

        outbox.send(TaskMessage::StateChange(TaskState::Running)).await?;
        let re = Regex::new(r"^(?<path>.+)$")?;
        let Some(caps) = re.captures(name) else {
            return Err(AutoschematicServerError {
                kind: AutoschematicServerErrorType::InvalidConnectorString(name.to_string()),
            }
            .into());
        };

        Ok(Box::new(PullRequestByTask {
            owner: owner.into(),
            repo: repo.into(),
            prefix: prefix.into(),
            task_addr: PathBuf::from(&caps["path"]),
            temp_dir: TempDir::new("autoschematic_pull_request_by_task")?,
            token,
            client,
            inbox,
            outbox,
        }))
    }

    async fn run(&mut self, _arg: serde_json::Value) -> anyhow::Result<()> {
        self.outbox.send(TaskMessage::StateChange(TaskState::Running)).await?;

        let _ = drain_inbox(&mut self.inbox).await.map_err(async |e| {
            tracing::error!("{}", e);
            let _ = self.outbox.send(TaskMessage::StateChange(TaskState::Stopped)).await;
        });

        let repo = self.client.repos(&self.owner, &self.repo).get().await?;

        let Some(default_branch) = repo.default_branch else {
            bail!("Repo {}/{} has no default branch", self.owner, self.repo)
        };

        // let head_ref = self
        //     .client
        //     .repos(&self.owner, &self.repo)
        //     .get_ref(&octocrab::params::repos::Reference::Branch(
        //         default_branch.clone(),
        //     ))
        //     .await?;
        clone_repo(&self.owner, &self.repo, self.temp_dir.path(), &default_branch, &self.token)
            .await
            .context("Cloning repo")?;

        let _ = drain_inbox(&mut self.inbox).await.map_err(async |e| {
            tracing::error!("{}", e);
            let _ = self.outbox.send(TaskMessage::StateChange(TaskState::Stopped)).await;
        });

        let repo_path = self.repo_path();

        let _chwd = ChangeWorkingDirectory::change(&repo_path)?;

        let _prefix = if self.prefix.is_absolute() {
            self.prefix.strip_prefix("/")?
        } else {
            &self.prefix
        };
        
        // autoschematic_core::workflow::task_exec ???
        // TODO Now we need a plain old connector task registry.
        // Oh, maybe this is something that belongs in connector_cache?
        // connector_cache or a separate implementation could handle coordinating the cluster (tasks and connectors, and locking...) 
        // and already serves as the cpu/mem reporter.
        // So, here, we want to take our master cache of tasks and insert a new task into it,
        // corresponding to whatever connector returns | FilterResponse::Task to our task_addr.
        // We want to wait until the task is done, and we want to get the results of its execution, 
        // like whatever files it wants to modify or secrets it wants to seal, merged for each phase.
        // Maybe results should be stored in a least-recently-used limited size cache...
        // Then, we're going to apply the file modifications,
        // create a new branch,
        // git add, git commit, git push,
        // create a pull request, (wait for the welcome message...)
        // if auto_plan, comment with the plan command
        // wait for the plan to complete
        // if plan requires approval before apply, wait for it...
        // if auto_apply, comment with the apply command
        // wait for the apply to complete
        // merge and close the PR

        // match self.test_type {
        //     TestType::Fuzz(ref path) => {
        //         self.run_fuzz_test(&path.clone()).await?;
        //     }
        // };

        Ok(())
    }
}

impl PullRequestByTask {
    pub fn repo_path(&self) -> PathBuf {
        self.temp_dir.path().join(&self.owner).join(&self.repo)
    }
}
