use std::time::Duration;

use crate::util::extract_template_message_type;
use anyhow::bail;
use autoschematic_core::task::{
    TaskInbox, TaskOutbox,
    message::{IssueComment, TaskMessage, TaskRegistryMessage},
};
use octocrab::models::webhook_events::{WebhookEvent, WebhookEventPayload, payload::IssueCommentWebhookEventAction};
use tokio::sync::mpsc::error::TryRecvError;

pub async fn wait_for_comment_types(
    owner: &str,
    repo: &str,
    issue: u64,
    message_types: &[&str],
    inbox: &mut TaskInbox,
) -> anyhow::Result<(String, IssueComment)> {
    loop {
        match inbox.recv().await {
            Some(TaskRegistryMessage::IssueComment(issue_comment)) => {
                tracing::error!("Got message! Type = {:?}", extract_template_message_type(&issue_comment.body));
                if let Some(comment_message_type) = extract_template_message_type(&issue_comment.body)? {
                    if issue_comment.issue == issue
                        && issue_comment.owner == owner
                        && issue_comment.repo == repo
                        && message_types.contains(&comment_message_type.as_str())
                    {
                        return Ok((comment_message_type, issue_comment));
                    } else {
                        tracing::error!("ignoring message type {:?}", comment_message_type)
                    }
                }
            }
            Some(TaskRegistryMessage::ShutDown) => {
                bail!("Shutting down task...")
            }
            None => {
                bail!("Shutting down task... (upstream channel closed)")
            }
        }
    }
}

pub async fn create_comment(outbox: &mut TaskOutbox, owner: &str, repo: &str, issue: u64, body: &str) -> anyhow::Result<()> {
    outbox
        .send(TaskMessage::IssueComment(IssueComment {
            owner: owner.to_string(),
            repo: repo.to_string(),
            issue,
            user: String::new(),
            body: body.to_string(),
        }))
        .await?;

    Ok(())
}

pub fn message_from_github_webhook(webhook_event: &WebhookEvent) -> anyhow::Result<Option<TaskRegistryMessage>> {
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
