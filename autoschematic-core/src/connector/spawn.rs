use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use crate::{
    config::Spec,
    connector::{Connector, ConnectorInbox},
    keystore::KeyStore,
};
use anyhow::{Context, bail};
#[cfg(feature = "python")]
use python::PythonConnector;

#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "sandbox")]
pub mod sandbox;

#[cfg(not(feature = "sandbox"))]
pub mod unsandbox;

pub async fn spawn_connector(
    shortname: &str,
    spec: &Spec,
    prefix: &Path,
    env: &HashMap<String, String>,
    // binary_cache: &BinaryCache,
    keystore: Option<Arc<dyn KeyStore>>,
) -> Result<(Arc<dyn Connector>, ConnectorInbox), anyhow::Error> {
    let (outbox, inbox) = tokio::sync::broadcast::channel(64);

    Ok((
        Arc::new(
            unsandbox::launch_server_binary(spec, shortname, prefix, env, outbox, keystore)
                .await
                .context("launch_server_binary()")?,
        ) as Arc<dyn Connector>,
        inbox,
    ))
}

pub async fn wait_for_socket(socket: &Path, timeout: Duration) -> anyhow::Result<()> {
    let start_time = std::time::Instant::now();

    loop {
        if std::time::Instant::now() - start_time > timeout {
            bail!("Timed out waiting for socket after {:?}", timeout)
        }
        if socket.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
