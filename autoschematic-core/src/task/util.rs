use std::time::Duration;

use anyhow::bail;
use tokio::sync::mpsc::error::TryRecvError;

use crate::task::{TaskInbox, message::TaskRegistryMessage};

pub async fn drain_inbox(inbox: &mut TaskInbox) -> anyhow::Result<()> {
    loop {
        let res = inbox.try_recv();
        tracing::error!("{:?}", res);
        match res {
            Ok(TaskRegistryMessage::ShutDown) => {
                bail!("Shutting down task...")
            }
            Err(TryRecvError::Empty) => return Ok(()),
            Err(e) => return Err(e.into()),
            _ => {}
        }
        tokio::time::sleep(Duration::from_secs(0)).await;
    }
}
