use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    thread::JoinHandle,
};

use crate::{
    bundle::UnbundleResponseElement,
    config::Spec,
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterResponse, GetDocResponse, GetResourceResponse, OpExecResponse,
        PlanResponseElement, SkeletonResponse, VirtToPhyResponse,
        handle::{ConnectorHandle, ConnectorHandleStatus},
        spawn::{random_error_dump_path, random_socket_path},
    },
    diag::DiagnosticResponse,
    grpc_bridge,
    keystore::KeyStore,
    tarpc_bridge::{self},
    util::passthrough_secrets_from_env,
};
use anyhow::bail;
use async_trait::async_trait;

use once_cell::sync::Lazy;
use process_wrap::tokio::*;
use sysinfo::{Pid, Process, ProcessRefreshKind, System};
use tokio::sync::Mutex;

/// This module handles unsandboxed execution of connector instances.
pub struct UnsandboxConnectorHandle {
    client: Arc<dyn Connector>,
    socket: PathBuf,
    error_dump: PathBuf,
    read_thread: Option<JoinHandle<()>>,
    child: Box<dyn process_wrap::tokio::TokioChildWrapper>,
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

    if let Some(spec_pre_command) = spec.pre_command()? {
        let mut command = TokioCommandWrap::with_new(spec_pre_command.binary, |command| {
            command.args(spec_pre_command.args);
            command.stderr(io::stderr());
            command.stdout(io::stderr());
            command.kill_on_drop(true);
        });

        #[cfg(unix)]
        {
            command.wrap(ProcessGroup::leader());
        }
        #[cfg(windows)]
        {
            command.wrap(JobObject);
        }

        let status = Box::into_pin(command.spawn()?.wait()).await?;

        if !status.success() {
            bail!(
                "launch_server_binary: pre_command failed: \n {:?} \n {:?}",
                command.command(),
                status.code()
            )
        }
    }

    let spec_command = spec.command()?;

    let mut command = TokioCommandWrap::with_new(spec_command.binary, |command| {
        command.args(spec_command.args);
        command.args([shortname.into(), prefix.into(), socket.clone(), error_dump.clone()]);
        command.stdout(io::stderr());
        command.stderr(io::stderr());
        command.kill_on_drop(true);

        for (key, val) in env {
            command.env(key, val);
        }
    });

    #[cfg(unix)]
    {
        command.wrap(ProcessGroup::leader());
    }
    #[cfg(windows)]
    {
        command.wrap(JobObject);
    }

    let child = command.spawn()?;

    tracing::info!("Launching client at {:?}", socket);

    let client = match spec.protocol() {
        crate::config::Protocol::Tarpc => tarpc_bridge::launch_client(&socket).await?,
        crate::config::Protocol::Grpc => grpc_bridge::launch_client(&socket).await?,
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

pub static SYSINFO: Lazy<Arc<Mutex<sysinfo::System>>> = Lazy::new(|| Arc::new(Mutex::new(sysinfo::System::new())));

#[async_trait]
impl ConnectorHandle for UnsandboxConnectorHandle {
    async fn status(&self) -> ConnectorHandleStatus {
        match self.child.inner().id() {
            Some(pid) => {
                let pid = Pid::from_u32(pid);

                SYSINFO.lock().await.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::Some(&[pid]),
                    false,
                    ProcessRefreshKind::nothing().with_cpu().with_memory(),
                );

                if let Some(p) = SYSINFO.lock().await.process(pid) {
                    return ConnectorHandleStatus::Alive {
                        memory: p.memory(),
                        cpu_usage: p.cpu_usage(),
                    };
                } else {
                    return ConnectorHandleStatus::Dead;
                }
            }
            None => ConnectorHandleStatus::Dead,
        }
    }
}
