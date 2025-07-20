use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    thread::JoinHandle,
    time::SystemTime,
};

use crate::{
    bundle::UnbundleResponseElement,
    config::Spec,
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterResponse, GetDocResponse, GetResourceResponse, OpExecResponse,
        PlanResponseElement, SkeletonResponse, VirtToPhyResponse,
    },
    diag::DiagnosticResponse,
    grpc_bridge,
    keystore::KeyStore,
    secret::SealedSecret,
    tarpc_bridge::{self},
    util::passthrough_secrets_from_env,
};
use anyhow::bail;
use async_trait::async_trait;

use rand::{Rng, distr::Alphanumeric};
use tokio::process::Child;
use walkdir::WalkDir;

/// This module handles unsandboxed execution of connector instances.
pub struct UnsandboxConnectorHandle {
    client: Arc<dyn Connector>,
    socket: PathBuf,
    error_dump: PathBuf,
    read_thread: Option<JoinHandle<()>>,
    child: Child,
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

#[async_trait]
impl Connector for UnsandboxConnectorHandle {
    async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> Result<Arc<dyn Connector>, anyhow::Error> {
        bail!("Connector::new() for UnsandboxConnectorHandle is a stub!")
        // <TarpcConnectorClient as Connector>::new(name, prefix, outbox).await
    }
    async fn init(&self) -> Result<(), anyhow::Error> {
        Connector::init(&self.client).await
    }

    async fn filter(&self, addr: &Path) -> Result<FilterResponse, anyhow::Error> {
        Connector::filter(&self.client, addr).await
    }

    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>> {
        Connector::list(&self.client, subpath).await
    }

    async fn subpaths(&self) -> anyhow::Result<Vec<PathBuf>> {
        Connector::subpaths(&self.client).await
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceResponse>, anyhow::Error> {
        Connector::get(&self.client, addr).await
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<PlanResponseElement>, anyhow::Error> {
        Connector::plan(&self.client, addr, current, desired).await
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecResponse, anyhow::Error> {
        Connector::op_exec(&self.client, addr, op).await
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyResponse, anyhow::Error> {
        Connector::addr_virt_to_phy(&self.client, addr).await
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        Connector::addr_phy_to_virt(&self.client, addr).await
    }

    async fn get_skeletons(&self) -> Result<Vec<SkeletonResponse>, anyhow::Error> {
        Connector::get_skeletons(&self.client).await
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> Result<Option<GetDocResponse>, anyhow::Error> {
        Connector::get_docstring(&self.client, addr, ident).await
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        Connector::eq(&self.client, addr, a, b).await
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<Option<DiagnosticResponse>, anyhow::Error> {
        Connector::diag(&self.client, addr, a).await
    }

    async fn unbundle(&self, addr: &Path, resource: &[u8]) -> Result<Vec<UnbundleResponseElement>, anyhow::Error> {
        Connector::unbundle(&self.client, addr, resource).await
    }
}

pub async fn launch_server_binary(
    spec: &Spec,
    shortname: &str,
    prefix: &Path,
    env: &HashMap<String, String>,
    outbox: ConnectorOutbox,
    keystore: Option<Arc<dyn KeyStore>>,
) -> anyhow::Result<UnsandboxConnectorHandle> {
    let mut env = env.clone();

    let socket = random_socket_path();
    let error_dump = random_error_dump_path();

    if let Some(keystore) = keystore {
        env = keystore.unseal_env_map(&env)?;
    } else {
        env = passthrough_secrets_from_env(&env)?;
    }

    let mut pre_command = None;

    let mut command = match spec {
        Spec::Binary { path, protocol } => {
            let mut binary_path = path.clone();
            if !binary_path.is_file() {
                binary_path = which::which(binary_path)?;
            }

            if !binary_path.is_file() {
                bail!("launch_server_binary: {}: not found", binary_path.display())
            }
            let mut command = tokio::process::Command::new(binary_path);
            let args = [shortname.into(), prefix.into(), socket.clone(), error_dump.clone()];
            command.args(args);
            command.stdout(io::stderr());
            command
        }
        Spec::Cargo { name, .. } => {
            let cargo_home = match std::env::var("CARGO_HOME") {
                Ok(p) => PathBuf::from(p),
                Err(_) => {
                    let Ok(home) = std::env::var("HOME") else {
                        bail!("$HOME not set!");
                    };
                    PathBuf::from(home).join(".cargo")
                }
            };

            // TODO Also parse `binary` and check .cargo/.cargo.toml
            let binary_path = cargo_home.join("bin").join(name);

            if !binary_path.is_file() {
                bail!("launch_server_binary: {}: not found", binary_path.display())
            }
            let mut command = tokio::process::Command::new(binary_path);
            let args = [shortname.into(), prefix.into(), socket.clone(), error_dump.clone()];
            command.args(args);
            command.stdout(io::stderr());
            command
        }
        Spec::CargoLocal {
            path, binary, features, ..
        } => {
            let manifest_path = path.join("Cargo.toml");
            if !manifest_path.is_file() {
                bail!("launch_server_binary: No Cargo.toml under {}", path.display())
            }

            let mut build_command = tokio::process::Command::new("cargo");
            build_command.kill_on_drop(true);

            build_command.args(["build", "--release", "--manifest-path", manifest_path.to_str().unwrap()]);
            if let Some(binary) = binary {
                build_command.args(["--bin", binary]);
            }
            if let Some(features) = features
                && !features.is_empty()
            {
                build_command.args(["--features", &features.join(",")]);
            }

            pre_command = Some(build_command);

            let mut command = tokio::process::Command::new("cargo");
            command.kill_on_drop(true);
            command.args(["run", "--release", "--manifest-path", manifest_path.to_str().unwrap()]);
            if let Some(binary) = binary {
                command.args(["--bin", binary]);
            }
            if let Some(features) = features
                && !features.is_empty()
            {
                command.args(["--features", &features.join(",")]);
            }
            command.args([String::from("--"), shortname.to_string()]);
            command.args([prefix, &socket, &error_dump]);
            command.stdout(io::stderr());
            command
        }
        Spec::TypescriptLocal { path } => {
            if !path.is_file() {
                bail!("launch_server_binary: {}: not found", path.display())
            }
            let mut command = tokio::process::Command::new("tsx");
            let args = [
                path.into(),
                shortname.into(),
                prefix.into(),
                socket.clone(),
                error_dump.clone(),
            ];
            command.args(args);
            command.stdout(io::stderr());
            command
        }
    };

    for (key, val) in env {
        command.env(key, val);
    }

    if let Some(mut pre_command) = pre_command {
        let output = pre_command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()
            .await?;

        if !output.status.success() {
            bail!("Pre-command failed: {:?}: {}", pre_command, output.status)
        }
    }

    let child = command.spawn()?;

    tracing::info!("Launching client at {:?}", socket);

    let client = match spec {
        Spec::Binary { protocol, .. } => match protocol {
            crate::config::Protocol::Tarpc => tarpc_bridge::launch_client(&socket).await?,
            crate::config::Protocol::Grpc => grpc_bridge::launch_client(&socket).await?,
        },
        Spec::Cargo { protocol, .. } => match protocol {
            crate::config::Protocol::Tarpc => tarpc_bridge::launch_client(&socket).await?,
            crate::config::Protocol::Grpc => grpc_bridge::launch_client(&socket).await?,
        },
        Spec::CargoLocal { protocol, .. } => match protocol {
            crate::config::Protocol::Tarpc => tarpc_bridge::launch_client(&socket).await?,
            crate::config::Protocol::Grpc => grpc_bridge::launch_client(&socket).await?,
        },
        Spec::TypescriptLocal { .. } => grpc_bridge::launch_client(&socket).await?,
    };

    tracing::info!("Launched client.");

    Ok(UnsandboxConnectorHandle {
        client: Arc::new(client),
        socket,
        error_dump,
        read_thread: None,
        child,
    })
}

impl Drop for UnsandboxConnectorHandle {
    fn drop(&mut self) {
        tracing::info!("DROP on UnsandboxConnectorHandle! Killing subprocess");
        self.child.start_kill().unwrap();
        self.child.try_wait().unwrap();

        match std::fs::remove_file(&self.socket) {
            Ok(_) => {}
            Err(e) => tracing::warn!("Couldn't remove socket {:?}: {}", self.socket, e),
        }

        match std::fs::remove_file(&self.error_dump) {
            Ok(_) => {}
            Err(e) => tracing::warn!("Couldn't remove error_dump {:?}: {}", self.error_dump, e),
        }

        if self.read_thread.is_some() {}
    }
}

pub fn unseal_secrets_to_folder(
    keystore: &Box<dyn KeyStore>,
    prefix: &Path,
    connector_shortname: &str,
    secret_mount: &Path,
) -> anyhow::Result<()> {
    for path in WalkDir::new(prefix.join(".secrets").join(connector_shortname))
        .into_iter()
        .filter_map(|entry| entry.ok())
        .map(|entry| PathBuf::from(entry.path()))
        .filter(|path| path.is_file())
    {
        tracing::error!("unseal_secrets: walk: {:?}", &path);
        let secret_file = std::fs::read_to_string(&path)?;
        let secrets: Vec<SealedSecret> = serde_json::from_str(&secret_file)?;
        let secret_text = keystore.unseal_secret(secrets.first().unwrap())?;
        let out_dir = secret_mount.join(prefix).join(connector_shortname);
        let out_path = path.strip_prefix(prefix.join(".secrets").join(connector_shortname))?;
        std::fs::create_dir_all(out_dir.join(out_path).parent().unwrap())?;
        std::fs::write(out_dir.join(out_path), secret_text)?;
    }
    Ok(())
}

fn seal_new_secrets_from_folder(
    keystore: impl KeyStore,
    prefix: &Path,
    domain: &str,
    connector_shortname: &str,
    newer_than: SystemTime,
    secret_mount: &Path,
) -> anyhow::Result<()> {
    // For each secret in the sandboxed connector's secret mount,
    // if it's newer than newer_than, seal it and write it to the repo.
    for path in WalkDir::new(secret_mount.join(connector_shortname))
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .metadata()
                .is_ok_and(|metadata| metadata.modified().is_ok_and(|system_time| system_time > newer_than))
        })
        .map(|entry| PathBuf::from(entry.path()))
        .filter(|path| path.is_file())
    {
        let secret_file = std::fs::read_to_string(&path)?;
        let mut sealed_secrets = Vec::new();
        for key_id in keystore.list()? {
            let sealed = keystore.seal_secret(domain, &key_id, &secret_file)?;
            sealed_secrets.push(sealed);
        }
        let sealed_json = serde_json::to_string_pretty(&sealed_secrets)?;
        std::fs::write(prefix.join(".secrets").join(connector_shortname).join(&path), sealed_json)?;
    }
    Ok(())
}
