// use octocrab::models::webhook_events::{
//     payload::IssueCommentWebhookEventAction, WebhookEvent, WebhookEventPayload,
// };
use serde::{Deserialize, Serialize};

use super::state::TaskState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    pub owner: String,
    pub repo: String,
    pub issue: u64,
    pub user: String,
    pub body: String,
}
/// Represents a message from the task registry to a task.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskRegistryMessage {
    // WebhookEvent(WebhookEvent),
    IssueComment(IssueComment),
    ShutDown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskMessage {
    StateChange(TaskState),
    IssueComment(IssueComment),
    LogLines(String),
}
