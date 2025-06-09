use std::path::PathBuf;

use indexmap::IndexMap;
use tokio::{sync::RwLock, task::JoinHandle};

use super::{state::TaskState, TaskRegistryBroadcast, TaskRegistryOutbox};


#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TaskRegistryKey {
    pub owner: String, 
    pub repo: String, 
    pub prefix: PathBuf, 
    pub task_name: String
}

#[derive(Debug)]
pub struct TaskRegistryEntry {
    // pub inbox: AgentRegistryInbox,
    pub broadcast: TaskRegistryBroadcast,
    pub outbox: TaskRegistryOutbox,
    // pub agent: Pin<Box<dyn Agent>>,
    pub join_handle: JoinHandle<anyhow::Result<()>>,
    
    pub state: TaskState
    
}

// pub trait AgentRegistry {
// }

#[derive(Debug)]
pub struct TaskRegistry {
    pub entries: RwLock<IndexMap<TaskRegistryKey, TaskRegistryEntry>>
}