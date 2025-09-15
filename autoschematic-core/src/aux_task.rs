use std::path::Path;

use async_trait::async_trait;
use message::{TaskMessage, TaskRegistryMessage};
// use test_task::TestTask;


pub mod message;
#[cfg(feature = "python")]
mod python_task;
pub mod registry;
pub mod state;
pub mod util;
#[cfg(feature = "python")]
use python_task::PythonTask;

pub type TaskOutbox = tokio::sync::mpsc::Sender<TaskMessage>;
pub type TaskInbox = tokio::sync::mpsc::Receiver<TaskRegistryMessage>;

pub type TaskRegistryOutbox = tokio::sync::mpsc::Sender<TaskRegistryMessage>;
pub type TaskRegistryInbox = tokio::sync::mpsc::Receiver<TaskMessage>;
pub type TaskRegistryBroadcast = tokio::sync::broadcast::Receiver<TaskMessage>;

#[async_trait]
pub trait Task: Send + Sync {
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
        Self: Sized;

    async fn run(&mut self, arg: serde_json::Value) -> anyhow::Result<()>;
}