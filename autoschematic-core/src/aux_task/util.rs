use std::time::Duration;

use anyhow::bail;
use tokio::sync::mpsc::error::TryRecvError;

use crate::aux_task::{TaskInbox, message::TaskRegistryMessage};

pub async fn drain_inbox(inbox: &mut TaskInbox) -> anyhow::Result<()> {
    loop {
        let res = inbox.try_recv();
        match res {
            Ok(TaskRegistryMessage::ShutDown) => {
                bail!("Shutting down task...")
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(e) => {
                tracing::error!("{:?}", e);
                return Err(e.into());
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_secs(0)).await;
    }
}
