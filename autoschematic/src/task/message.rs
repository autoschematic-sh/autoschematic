use anyhow::Ok;
use octocrab::models::webhook_events::{
    payload::IssueCommentWebhookEventAction, WebhookEvent, WebhookEventPayload,
};
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

pub fn from_github_webhook(
    webhook_event: &WebhookEvent,
) -> anyhow::Result<Option<TaskRegistryMessage>> {
    // {
    //     let agent_registry = agent_registry.entries.read().await;
    //     for (k, v) in &*agent_registry {
    //         // oh brother that's gonna be a big clone
    //         // remember graphql? lol
    //         match v.outbox.try_send(AgentRegistryMessage::WebhookEvent(webhook_event.clone())) {
    //             Ok(_) => {}
    //             Err(e) => {
    //                 tracing::error!("Failed to send message to agent: {}: {:#}", k.agent_name, e)
    //             }
    //         }
    //     }
    // }
    match webhook_event.specific {
        WebhookEventPayload::IssueComment(ref payload)
            if payload.action == IssueCommentWebhookEventAction::Created
                || payload.action == IssueCommentWebhookEventAction::Edited =>
        {
            let comment_username = payload.comment.user.login.clone();
            let _comment_id = payload.comment.id;
            let Some(ref comment_body) = payload.comment.body else {
                return Ok(None);
            };

            let Some(ref repository) = webhook_event.repository else {
                return Ok(None);
            };

            let Some(ref author) = repository.owner else {
                return Ok(None);
            };

            let _comment_url = &payload.comment.html_url;

            Ok(Some(TaskRegistryMessage::IssueComment(IssueComment {
                owner: author.login.clone(),
                repo: repository.name.clone(),
                issue: payload.issue.number,
                user: comment_username,
                body: comment_body.clone(),
            })))
        }
        _ => Ok(None),
    }
}
