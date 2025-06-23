use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use async_trait::async_trait;
use octocrab::{Octocrab, models::InstallationId};
use regex::Regex;
use secrecy::SecretBox;
use tempdir::TempDir;

mod fuzz_test;
mod util;

use crate::{
    chwd::ChangeWorkingDirectory,
    credentials,
    error::{AutoschematicServerError, AutoschematicServerErrorType},
};

use autoschematic_core::{
    git_util::clone_repo,
    task::{Task, TaskInbox, TaskOutbox, message::TaskMessage, state::TaskState, util::drain_inbox},
};

pub enum TestType {
    Fuzz(PathBuf),
}

pub struct TestTask {
    pub owner: String,
    pub repo: String,
    prefix: PathBuf,
    pub test_type: TestType,
    temp_dir: TempDir,
    token: SecretBox<str>,
    pub client: Octocrab,
    inbox: TaskInbox,
    outbox: TaskOutbox,
}

#[async_trait]
impl Task for TestTask {
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
        let re = Regex::new(r"^(?<type>[^:]+):(?<path>.+)$")?;
        let Some(caps) = re.captures(name) else {
            return Err(AutoschematicServerError {
                kind: AutoschematicServerErrorType::InvalidConnectorString(name.to_string()),
            }
            .into());
        };

        let test_type = match &caps["type"] {
            "fuzz" => TestType::Fuzz(caps["path"].into()),
            _ => bail!("TestTask: no such test type: {}", &caps["type"]),
        };

        Ok(Box::new(TestTask {
            owner: owner.into(),
            repo: repo.into(),
            prefix: prefix.into(),
            test_type,
            temp_dir: TempDir::new("autoschematic_task")?,
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

        // match &head_ref.object {
        //     octocrab::models::repos::Object::Commit { sha, url } => {
        //         clone_repo(
        //             &self.owner,
        //             &self.repo,
        //             self.temp_dir.path(),
        //             sha,
        //             &self.token,
        //         )
        //         .await
        //         .context("Cloning repo")?;
        //     }
        //     octocrab::models::repos::Object::Tag { sha, url } => {
        //         clone_repo(
        //             &self.owner,
        //             &self.repo,
        //             self.temp_dir.path(),
        //             sha,
        //             &self.token,
        //         )
        //         .await
        //         .context("Cloning repo")?;
        //     }
        //     _ => todo!(),
        // }

        // let rand_suffix: String = rand::thread_rng()
        //     .sample_iter(&Alphanumeric)
        //     .take(20)
        //     .map(char::from)
        //     .collect();

        // let branch_name = match &self.test_type {
        //     TestType::Fuzz(path) => {
        //         format!(
        //             "test-fuzz/{}-{}",
        //             path.to_str().unwrap_or_default(),
        //             rand_suffix
        //         )
        //     }
        // };

        let repo_path = self.repo_path();

        let _chwd = ChangeWorkingDirectory::change(&repo_path)?;

        let _prefix = if self.prefix.is_absolute() {
            self.prefix.strip_prefix("/")?
        } else {
            &self.prefix
        };

        match self.test_type {
            TestType::Fuzz(ref path) => {
                self.run_fuzz_test(&path.clone()).await?;
            }
        };

        Ok(())
    }
}

impl TestTask {
    pub fn repo_path(&self) -> PathBuf {
        self.temp_dir.path().join(&self.owner).join(&self.repo)
    }
}
