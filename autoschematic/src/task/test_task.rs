use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use async_trait::async_trait;
use regex::Regex;

mod fuzz_test;

use autoschematic_core::{
    error::{AutoschematicError, AutoschematicErrorType},
    git_util::clone_repo,
    task::{Task, TaskInbox, TaskOutbox, message::TaskMessage, state::TaskState, util::drain_inbox},
};

pub enum TestType {
    Fuzz(PathBuf),
}

pub struct TestTask {
    prefix: PathBuf,
    pub test_type: TestType,
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
        outbox.send(TaskMessage::StateChange(TaskState::Running)).await?;
        let re = Regex::new(r"^(?<type>[^:]+):(?<path>.+)$")?;
        let Some(caps) = re.captures(name) else {
            return Err(AutoschematicError {
                kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
            }
            .into());
        };

        let test_type = match &caps["type"] {
            "fuzz" => TestType::Fuzz(caps["path"].into()),
            _ => bail!("TestTask: no such test type: {}", &caps["type"]),
        };

        Ok(Box::new(TestTask {
            prefix: prefix.into(),
            test_type,
            inbox,
            outbox,
        }))
    }

    async fn run(&mut self, _arg: serde_json::Value) -> anyhow::Result<()> {
        self.outbox.send(TaskMessage::StateChange(TaskState::Running)).await?;

        let _ = drain_inbox(&mut self.inbox).await.map_err(async |e| {
            let _ = self.outbox.send(TaskMessage::StateChange(TaskState::Stopped)).await;
        });

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
