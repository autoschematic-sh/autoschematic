use std::time::Duration;

use anyhow::bail;
use tokio::sync::mpsc::error::TryRecvError;
use crate::util::extract_template_message_type;

use super::message::TaskRegistryMessage;
use super::TaskOutbox;
use super::{message::IssueComment, TaskInbox};

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
                tracing::error!(
                    "Got message! Type = {:?}",
                    extract_template_message_type(&issue_comment.body)
                );
                if let Some(comment_message_type) =
                    extract_template_message_type(&issue_comment.body)?
                {
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

pub async fn drain_inbox(inbox: &mut TaskInbox) -> anyhow::Result<()> {
    loop {
        let res = inbox.try_recv();
        tracing::error!("{:?}", res);
        match res {
            Ok(super::message::TaskRegistryMessage::ShutDown) => {
                bail!("Shutting down task...")
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(e) => return Err(e.into()),
            _ => {}
        }
        tokio::time::sleep(Duration::from_secs(0)).await;
    }
}

pub async fn create_comment(
    outbox: &mut TaskOutbox,
    owner: &str,
    repo: &str,
    issue: u64,
    body: &str,
) -> anyhow::Result<()> {
    outbox
        .send(super::message::TaskMessage::IssueComment(IssueComment {
            owner: owner.to_string(),
            repo: repo.to_string(),
            issue: issue,
            user: String::new(),
            body: body.to_string(),
        }))
        .await?;

    Ok(())
}
