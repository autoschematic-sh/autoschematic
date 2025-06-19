use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    thread::JoinHandle,
    time::SystemTime,
};

use crate::{
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterOutput, GetDocOutput, GetResourceOutput, OpExecOutput, OpPlanOutput,
        SkeletonOutput, VirtToPhyOutput,
    },
    diag::DiagnosticOutput,
    keystore::KeyStore,
    secret::SealedSecret,
    tarpc_bridge::{TarpcConnectorClient, launch_client},
    util::passthrough_secrets_from_env,
};
use anyhow::bail;
use async_trait::async_trait;

// use nix::{
//     errno::Errno,
//     sched::CloneFlags,
//     sys::signal::Signal::SIGKILL,
//     sys::signal::kill,
//     unistd::{Pid, Uid, execve, getegid, geteuid, pipe, setresuid},
// };
use rand::{Rng, distr::Alphanumeric};
use tokio::process::Child;
use walkdir::WalkDir;

/// This module handles unsandboxed execution of connector instances.
pub struct UnsandboxConnectorHandle {
    client: TarpcConnectorClient,
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
    async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> Result<Box<dyn Connector>, anyhow::Error> {
        <TarpcConnectorClient as Connector>::new(name, prefix, outbox).await
    }
    async fn init(&self) -> Result<(), anyhow::Error> {
        Connector::init(&self.client).await
    }

    async fn filter(&self, addr: &Path) -> Result<FilterOutput, anyhow::Error> {
        Connector::filter(&self.client, addr).await
    }

    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>> {
        Connector::list(&self.client, subpath).await
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceOutput>, anyhow::Error> {
        Connector::get(&self.client, addr).await
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error> {
        Connector::plan(&self.client, addr, current, desired).await
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecOutput, anyhow::Error> {
        Connector::op_exec(&self.client, addr, op).await
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyOutput, anyhow::Error> {
        Connector::addr_virt_to_phy(&self.client, addr).await
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        Connector::addr_phy_to_virt(&self.client, addr).await
    }

    async fn get_skeletons(&self) -> Result<Vec<SkeletonOutput>, anyhow::Error> {
        Connector::get_skeletons(&self.client).await
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> Result<Option<GetDocOutput>, anyhow::Error> {
        Connector::get_docstring(&self.client, addr, ident).await
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        Connector::eq(&self.client, addr, a, b).await
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<DiagnosticOutput, anyhow::Error> {
        Connector::diag(&self.client, addr, a).await
    }
}

pub async fn launch_server_binary(
    binary: &Path,
    name: &str,
    prefix: &Path,
    env: &HashMap<String, String>,
    outbox: ConnectorOutbox,
    keystore: Option<&Box<dyn KeyStore>>,
) -> anyhow::Result<UnsandboxConnectorHandle> {
    let mut env = env.clone();
    let mut binary = PathBuf::from(binary);

    if !binary.is_file() {
        binary = which::which(binary)?;
    }

    if !binary.is_file() {
        bail!("launch_server_binary: {}: not found", binary.display())
    }

    let socket = random_socket_path();
    let error_dump = random_error_dump_path();

    if let Some(keystore) = keystore {
        env = keystore.unseal_env_map(&env)?;
    } else {
        env = passthrough_secrets_from_env(&env)?;
    }

    let args = [name.into(), prefix.into(), socket.clone(), error_dump.clone()];
    let mut command = &mut tokio::process::Command::new(binary);

    command = command.args(args);

    for (key, val) in env {
        command = command.env(key, val);
    }

    let child = command.spawn()?;

    tracing::info!("Launching client at {:?}", socket);

    let client = launch_client(&socket).await?;
    tracing::info!("Launched client.");

    Ok(UnsandboxConnectorHandle {
        client,
        socket,
        error_dump,
        read_thread: None,
        child,
    })
}

impl Drop for UnsandboxConnectorHandle {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.socket) {
            Ok(_) => {}
            Err(e) => tracing::warn!("Couldn't remove socket {:?}: {}", self.socket, e),
        }

        match std::fs::remove_file(&self.error_dump) {
            Ok(_) => {}
            Err(e) => tracing::warn!("Couldn't remove error_dump {:?}: {}", self.error_dump, e),
        }

        if self.read_thread.is_some() {
            // handle.
        }

        tracing::info!("DROP on UnsandboxConnectorHandle! Killing subprocess");
        self.child.start_kill().unwrap();
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
