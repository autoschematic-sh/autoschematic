use std::{
    collections::HashMap,
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use crate::util::repo_root;
use crate::{
    config::Spec,
    connector::{ConnectorInbox, handle::ConnectorHandle},
    keystore::KeyStore,
};
use anyhow::{Context, bail};
use rand::{Rng, distr::Alphanumeric};

#[cfg(target_os = "linux")]
pub mod sandbox;

pub mod unsandbox;

#[cfg(target_os = "linux")]
/// On Linux, the sandbox can be opted into by setting AUTOSCHEMATIC_SANDBOX=true
/// See autoschematic-core/src/connector/spawn/sandbox.rs for the sandboxing implementation.
pub fn is_sandbox_enabled() -> bool {
    match std::env::var("AUTOSCHEMATIC_SANDBOX") {
        Ok(s) if s == "true" => true,
        Ok(s) if s == "false" => false,
        Ok(_) => true,
        Err(_) => false,
    }
}

pub fn have_squashfs() -> Option<PathBuf> {
    match std::env::var("AUTOSCHEMATIC_SANDBOX_ROOT") {
        Ok(s) => Some(PathBuf::from(s)),
        Err(_) => None,
    }
}

pub async fn spawn_connector(
    shortname: &str,
    spec: &Spec,
    prefix: &Path,
    env: &HashMap<String, String>,
    keystore: Option<Arc<dyn KeyStore>>,
) -> Result<(Arc<dyn ConnectorHandle>, ConnectorInbox), anyhow::Error> {
    let (outbox, inbox) = tokio::sync::broadcast::channel(64);

    create_dir_all("/tmp/autoschematic")?;

    #[cfg(target_os = "linux")]
    return Ok((
        if is_sandbox_enabled() {
            let new_root = std::env::var("AUTOSCHEMATIC_SANDBOX_ROOT")?;
            let repo_path = repo_root()?.canonicalize()?;
            Arc::new(
                sandbox::launch_server_binary_sandboxed(
                    spec,
                    shortname,
                    prefix,
                    env,
                    outbox,
                    keystore,
                    new_root.into(),
                    repo_path,
                )
                .await
                .context("launch_server_binary_sandboxed()")?,
            ) as Arc<dyn ConnectorHandle>
        } else {
            Arc::new(
                unsandbox::launch_server_binary(spec, shortname, prefix, env, outbox, keystore)
                    .await
                    .context("launch_server_binary()")?,
            ) as Arc<dyn ConnectorHandle>
        },
        inbox,
    ));

    #[cfg(not(target_os = "linux"))]
    return Ok((
        Arc::new(
            unsandbox::launch_server_binary(spec, shortname, prefix, env, outbox, keystore)
                .await
                .context("launch_server_binary()")?,
        ) as Arc<dyn ConnectorHandle>,
        inbox,
    ));
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
        let socket_id: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();
        let socket = PathBuf::from(format!("/tmp/autoschematic/{}.sock", socket_id));

        if let Ok(false) = socket.try_exists() {
            tracing::info!("Creating socket at {:?}", socket);
            return socket;
        }
    }
}

fn random_error_dump_path() -> PathBuf {
    loop {
        let dump_id: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

        let dump = PathBuf::from(format!("/tmp/autoschematic/{}.dump", dump_id));

        if let Ok(false) = dump.try_exists() {
            return dump;
        }
    }
}

fn random_overlay_dir(_root: &Path) -> PathBuf {
    loop {
        let overlay_s: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

        let mut overlay = Path::new("/tmp/").join(overlay_s);

        overlay.set_extension("overlay");

        if let Ok(false) = overlay.try_exists() {
            return overlay;
        }
    }
}
