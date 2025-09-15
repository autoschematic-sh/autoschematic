use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, bail};
// use indexmap::IndexMap;
use tokio::{sync::RwLock, task::JoinHandle};

use crate::aux_task::message::{TaskMessage, TaskRegistryMessage};

use super::{TaskRegistryBroadcast, TaskRegistryOutbox, state::TaskState};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TaskRegistryKey {
    pub owner: String,
    pub repo: String,
    pub prefix: PathBuf,
    pub task_name: String,
}

#[derive(Debug)]
pub struct TaskRegistryEntry {
    // pub inbox: AgentRegistryInbox,
    pub broadcast: TaskRegistryBroadcast,
    pub outbox: TaskRegistryOutbox,
    // pub agent: Pin<Box<dyn Agent>>,
    pub join_handle: JoinHandle<anyhow::Result<()>>,

    pub state: TaskState,
}

// pub trait AgentRegistry {
// }

#[derive(Debug, Default)]
pub struct TaskRegistry {
    pub entries: RwLock<HashMap<TaskRegistryKey, TaskRegistryEntry>>,
}

impl TaskRegistry {
    pub async fn try_send_message(&self, registry_key: &TaskRegistryKey, message: TaskRegistryMessage) -> anyhow::Result<()> {
        let registry = self.entries.read().await;

        if let Some(task) = registry.get(registry_key) {
            tracing::error!("Try send: {:?}", message);
            Ok(task
                .outbox
                .try_send(message)
                .context(format!("Sending message to task {registry_key:?}"))?)
        } else {
            bail!("Task not found for key {:?}", registry_key)
        }
    }

    pub async fn subscribe_task_state(
        &self,
        registry_key: &TaskRegistryKey,
    ) -> anyhow::Result<tokio::sync::broadcast::Receiver<TaskMessage>> {
        let registry = self.entries.read().await;

        if let Some(task) = registry.get(registry_key) {
            Ok(task.broadcast.resubscribe())
        } else {
            bail!("Task not found for key {:?}", registry_key)
        }
    }
}
