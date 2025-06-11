use std::{
    collections::HashMap,
    ffi::{CString, OsStr, OsString},
    fs::create_dir_all,
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
    error::ErrorMessage,
    keystore::KeyStore,
    secret::SealedSecret,
    tarpc_bridge::{TarpcConnectorClient, launch_client},
    util::passthrough_secrets_from_env,
};
use anyhow::{Context, bail};
use async_trait::async_trait;

// use nix::{
//     errno::Errno,
//     sched::CloneFlags,
//     sys::signal::Signal::SIGKILL,
//     sys::signal::kill,
//     unistd::{Pid, Uid, execve, getegid, geteuid, pipe, setresuid},
// };
use rand::{Rng, distr::Alphanumeric};
use tokio::{process::Child, sync::mpsc::Receiver};
use walkdir::WalkDir;

/// This module handles sandboxing of connector instances using Linux-kernel specific
/// methods, such as cgroups and namespaces.
pub struct UnsandboxConnectorHandle {
    client: TarpcConnectorClient,
    socket: PathBuf,
    error_dump: PathBuf,
    read_thread: Option<JoinHandle<()>>,
    child: Child,
}

impl UnsandboxConnectorHandle {
    // pub fn still_alive(&self) -> anyhow::Result<()> {
    //     // if kill(self.pid, None).is_ok() {
    //     //     Ok(())
    //     // } else {
    //     //     if self.error_dump.is_file() {
    //     //         match std::fs::read_to_string(&self.error_dump) {
    //     //             Ok(dump) => Err(ErrorMessage { msg: dump }.into()),
    //     //             Err(e) => {
    //     //                 bail!("Connector process exited, failed to read error dump: {}", e)
    //     //             }
    //     //         }
    //     //     } else {
    //     //         bail!("Connector process exited without any error dump!")
    //     //     }
    //     // }
    // }
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
        let res = Connector::init(&self.client).await;
        res
    }

    async fn filter(&self, addr: &Path) -> Result<FilterOutput, anyhow::Error> {
        let res = Connector::filter(&self.client, addr).await;
        res
    }

    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let res = Connector::list(&self.client, subpath).await;
        res
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceOutput>, anyhow::Error> {
        let res = Connector::get(&self.client, addr).await;
        res
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error> {
        let res = Connector::plan(&self.client, addr, current, desired).await;
        res
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecOutput, anyhow::Error> {
        let res = Connector::op_exec(&self.client, addr, op).await;
        res
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyOutput, anyhow::Error> {
        let res = Connector::addr_virt_to_phy(&self.client, addr).await;
        res
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        let res = Connector::addr_phy_to_virt(&self.client, addr).await;
        res
    }

    async fn get_skeletons(&self) -> Result<Vec<SkeletonOutput>, anyhow::Error> {
        let res = Connector::get_skeletons(&self.client).await;
        res
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> Result<Option<GetDocOutput>, anyhow::Error> {
        let res = Connector::get_docstring(&self.client, addr, ident).await;
        res
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        let res = Connector::eq(&self.client, addr, a, b).await;
        res
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<DiagnosticOutput, anyhow::Error> {
        let res = Connector::diag(&self.client, addr, a).await;
        res
    }
}

// pub async fn launch_server_sandboxed<C: Connector>(
//     name: &str,
//     prefix: &Path,
//     outbox: Sender<Option<String>>,
//     keystore: Option<Box<dyn KeyStore>>,
// ) -> anyhow::Result<SandboxConnectorHandle> {
//     let socket = random_socket_path();
//     let error_dump = random_error_dump_path();

//     let mut flags = CloneFlags::CLONE_NEWUTS;
//     flags.insert(CloneFlags::CLONE_NEWNS);
//     flags.insert(CloneFlags::CLONE_NEWCGROUP);
//     // flags.insert(CloneFlags::CLONE_NEWNET);
//     // flags.insert(CloneFlags::CLONE_NEWUSER);
//     // flags.insert(CloneFlags::CLONE_NEWPID);

//     let mut pid: Option<Pid> = None;
//     unsafe {
//         const STACK_SIZE: usize = 1024 * 1024;
//         let ref mut stack = [0; STACK_SIZE];

//         let res = nix::sched::clone(
//             Box::new(async || init_server::<C>(name, prefix, &socket, outbox.clone()).await.unwrap()),
//             stack,
//             flags,
//             None,
//         )?;
//         pid = Some(res);
//     }

//     let Some(pid) = pid else {
//         bail!("launch_server_sandboxed: Failed to clone() process")
//     };

//     let client = launch_client(&socket).await?;

//     Ok(SandboxConnectorHandle {
//         client,
//         socket: socket.to_path_buf(),
//         error_dump,
//         read_thread: None,
//         pid: pid,
//     })
// }
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
        bail!("launch_server_binary_sandboxed: {}: not found", binary.display())
    }

    let socket = random_socket_path();
    let error_dump = random_error_dump_path();

    if let Some(keystore) = keystore {
        env = keystore.unseal_env_map(&env)?;
    } else {
        env = passthrough_secrets_from_env(&env)?;
    }

    tracing::info!("Launching client at {:?}", socket);

    let client = launch_client(&socket).await?;
    tracing::info!("Launched client.");

    let args = [binary.clone(), name.into(), prefix.into(), socket.clone(), error_dump.clone()];
    let mut command = &mut tokio::process::Command::new(binary);

    command = command.args(args);

    for (key, val) in env {
        command = command.env(key, val);
    }

    let child = command.spawn()?;

    return Ok(UnsandboxConnectorHandle {
        client,
        socket,
        error_dump,
        read_thread: None,
        child,
    });
}

#[cfg(feature = "sandbox")]
pub async fn launch_server_binary_sandboxed(
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
        bail!("launch_server_binary_sandboxed: {}: not found", binary.display())
    }

    let socket = random_socket_path();
    let error_dump = random_error_dump_path();

    if let Some(keystore) = keystore {
        env = keystore.unseal_env_map(&env)?;
    } else {
        env = passthrough_secrets_from_env(&env)?;
    }

    //
    // The cloned child is started in a new mount namespace
    let mut flags = CloneFlags::CLONE_NEWNS;
    // let mut flags = CloneFlags::CLONE_NEWNS;
    // Create the process in a new cgroup namespace.
    flags.insert(CloneFlags::CLONE_NEWCGROUP);
    // Create the process in a new IPC namespace.
    flags.insert(CloneFlags::CLONE_NEWIPC);
    // Create the process in a new user namespace.
    flags.insert(CloneFlags::CLONE_NEWUSER);
    // Create the process in a new PID namespace.
    flags.insert(CloneFlags::CLONE_NEWPID);
    // Create the process in a new UTS namespace.
    flags.insert(CloneFlags::CLONE_NEWUTS);

    let (stdout_r, stdout_w) = pipe()?;
    let (stderr_r, stderr_w) = pipe()?;

    let (stdout_r, stdout_w) = (stdout_r, stdout_w);
    let (stderr_r, stderr_w) = (stderr_r, stderr_w);

    tracing::info!("Launching sandboxed binary at {:?}", binary);

    let mut pid: Option<Pid> = None;
    unsafe {
        const STACK_SIZE: usize = 4 * 1024 * 1024;
        let mut stack = vec![0_u8; STACK_SIZE];

        // let secret_mount_path = PathBuf::from("/tmp/secrets").join(&prefix).join(&name);
        let secret_mount_path = PathBuf::from("/tmp/secrets");

        let Some(binary) = binary.to_str() else {
            bail!("Binary path {:#?} not valid UTF-8", binary)
        };

        let Some(prefix) = prefix.to_str() else {
            bail!("Prefix {:#?} not valid UTF-8", prefix)
        };

        let Some(socket) = socket.to_str() else {
            bail!("Socket {:#?} not valid UTF-8", socket)
        };

        let Some(error_dump) = error_dump.to_str() else {
            bail!("Error dump {:#?} not valid UTF-8", error_dump)
        };

        let binary_c = CString::new(String::from(binary))?;
        let name_c = CString::new(String::from(name))?;
        let prefix_c = CString::new(String::from(prefix))?;
        let socket_c = CString::new(String::from(socket))?;
        let error_dump_c = CString::new(String::from(error_dump))?;

        let args = [binary_c.clone(), name_c, prefix_c, socket_c, error_dump_c.clone()];
        let mut c_env: Vec<CString> = Vec::new();

        for (key, value) in &env {
            c_env.push(CString::new(format!("{}={}", key, value))?)
        }

        let uid = geteuid();
        let gid = getegid();

        let res = nix::sched::clone(
            Box::new(|| {
                std::fs::write("/proc/self/uid_map", format!("0 {} 1", uid)).expect("Couldn't write to /proc/self/uid_map!");

                std::fs::write("/proc/self/setgroups", "deny").expect("Couldn't write to /proc/self/setgroups!");

                std::fs::write("/proc/self/gid_map", format!("0 {} 1", gid)).expect("Couldn't write to /proc/self/gid_map!");

                // Probably the overflow UID, right?
                let old_uid = geteuid();

                setresuid(Uid::from_raw(0), Uid::from_raw(0), Uid::from_raw(0)).expect("Couldn't setuid to 0 in sandbox");

                create_dir_all(&secret_mount_path).expect("couldn't create mount dir");

                nix::mount::mount(
                    None::<&PathBuf>,
                    &secret_mount_path,
                    Some("tmpfs"),
                    nix::mount::MsFlags::empty(),
                    Some("size=64m,mode=0777"),
                )
                .expect("couldn't create mount");

                // std::fs::write(secret_mount_path.join("test"), "ASS!")
                //     .expect("couldn't write secret");
                //
                if let Some(keystore) = &keystore {
                    unseal_secrets_to_folder(&keystore, &PathBuf::from(prefix), name, &secret_mount_path)
                        .expect("Failed to unseal secrets to connector mount");
                }

                setresuid(old_uid, old_uid, old_uid).expect(&format!("Couldn't setuid to {:?} in sandbox", old_uid));

                // close(stdout_r).ok();
                // close(stderr_r).ok();

                // // Redirect stdout/stderr to our pipes
                // dup2(stdout_w, libc::STDOUT_FILENO).unwrap();
                // dup2(stderr_w, libc::STDERR_FILENO).unwrap();

                // // We don't need the write ends open after dup2
                // close(stdout_w).ok();
                // close(stderr_w).ok();
                // let ls = std::fs::read_dir(&secret_mount_path);
                // tracing::error!("ls = {:?}", ls.iter().map(|s| format!("{:?}", s)).collect::<Vec<String>>());

                execve(&binary_c, &args, &c_env).unwrap();
                0
            }),
            &mut stack,
            flags,
            None,
        )?;
        pid = Some(res);

        tracing::debug!("Launched connector {:#?} at PID {:#?}", binary_c, pid);
    }

    let stdout_outbox = outbox.clone();
    let stdout_thread = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 1024];
        loop {
            match nix::unistd::read(&stdout_r, &mut buf) {
                Ok(0) => {
                    // 0 means EOF, so the child closed its end or exited. We're done.
                    break;
                }
                Ok(n) => {
                    // Process `n` bytes from `buf`
                    // e.g. log it, send it somewhere, ...
                    let n = std::cmp::min(1024, n);
                    // let outbox = stdout_outbox.read().aait;
                    stdout_outbox
                        .send(Some(String::from_utf8_lossy(&buf[0..n]).to_string()))
                        .unwrap();
                }
                Err(Errno::EAGAIN) => {}
                Err(err) => {
                    // Some I/O error occurred
                    tracing::error!("Reader error: {}", err);
                    break;
                }
            }
        }
    });

    let stderr_outbox = outbox.clone();
    let stderr_thread = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 1024];
        loop {
            match nix::unistd::read(&stderr_r, &mut buf) {
                Ok(0) => {
                    // 0 means EOF, so the child closed its end or exited. We're done.
                    break;
                }
                Ok(n) => {
                    // Process `n` bytes from `buf`
                    // e.g. log it, send it somewhere, ...
                    let n = std::cmp::min(1024, n);
                    // let outbox = stderr_outbox.read().await;
                    stderr_outbox
                        .send(Some(String::from_utf8_lossy(&buf[0..n]).to_string()))
                        .unwrap();
                }
                Err(err) => {
                    // Some I/O error occurred
                    tracing::error!("Reader error: {}", err);
                    break;
                }
            }
        }
    });

    tracing::info!("Launching client at {:?}", socket);

    let client = launch_client(&socket).await?;
    tracing::info!("Launched client.");

    return Ok(UnsandboxConnectorHandle {
        client,
        socket: socket.to_path_buf(),
        error_dump,
        read_thread: None,
        pid: pid.unwrap(),
    });
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

        if let Some(_) = &self.read_thread {
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
