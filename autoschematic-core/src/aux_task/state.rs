use serde::{Deserialize, Serialize};



#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskState {
    #[default]
    Stopped,
    Running,
    Succeeded,
    Error{ message: String},
}