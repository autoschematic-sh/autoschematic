use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::{
    config::Spec,
    connector::{Connector, ConnectorInbox, handle::ConnectorHandle},
    keystore::KeyStore,
};
use anyhow::{Context, bail};
use rand::{Rng, distr::Alphanumeric};

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
) -> Result<(Arc<dyn ConnectorHandle>, ConnectorInbox), anyhow::Error> {
    let (outbox, inbox) = tokio::sync::broadcast::channel(64);

    Ok((
        Arc::new(
            #[cfg(feature = "sandbox")]
            sandbox::launch_server_binary_sandboxed(spec, shortname, prefix, env, outbox, keystore)
                .await
                .context("launch_server_binary()")?,
            #[cfg(not(feature = "sandbox"))]
            unsandbox::launch_server_binary(spec, shortname, prefix, env, outbox, keystore)
                .await
                .context("launch_server_binary()")?,
        ) as Arc<dyn ConnectorHandle>,
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

fn random_socket_path() -> PathBuf {
    loop {
        let socket_s: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

        let mut socket = PathBuf::from("/tmp/").join(socket_s);

        socket.set_extension("sock");

        if let Ok(false) = socket.try_exists() {
            tracing::info!("Creating socket at {:?}", socket);
            return socket;
        }
    }
}

fn random_error_dump_path() -> PathBuf {
    loop {
        let dump_s: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

        let mut dump = PathBuf::from("/tmp/").join(dump_s);

        dump.set_extension("dump");

        if let Ok(false) = dump.try_exists() {
            return dump;
        }
    }
}
