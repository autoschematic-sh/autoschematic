use std::{
    collections::HashMap,
    ffi::{CString, OsString},
    fs::create_dir_all,
    os::{fd::OwnedFd, unix::ffi::OsStringExt},
    path::{Path, PathBuf},
    sync::Arc,
    thread::JoinHandle,
    time::SystemTime,
};

use crate::{
    bundle::UnbundleResponseElement,
    config::Spec,
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterResponse, GetDocResponse, GetResourceResponse, OpExecResponse,
        PlanResponseElement, SkeletonResponse, TaskExecResponse, VirtToPhyResponse,
        handle::{ConnectorHandle, ConnectorHandleStatus},
        spawn::{random_error_dump_path, random_socket_path},
    },
    diag::DiagnosticResponse,
    error::ErrorMessage,
    grpc_bridge,
    keystore::KeyStore,
    secret::SealedSecret,
    tarpc_bridge::{self},
    util::passthrough_secrets_from_env,
};
use anyhow::{Context, bail};
use async_trait::async_trait;

use git2::IntoCString;
use nix::{
    errno::Errno,
    mount::{MntFlags, MsFlags, umount2},
    sched::CloneFlags,
    sys::signal::{Signal::SIGKILL, kill, killpg},
    unistd::{Gid, Pid, Uid, chdir, execve, getegid, geteuid, pipe, pivot_root, setresgid, setresuid},
};
use once_cell::sync::Lazy;
use rand::{Rng, distr::Alphanumeric};
use sysinfo::ProcessRefreshKind;
use tokio::sync::Mutex;
use walkdir::WalkDir;

/// This module handles sandboxing of connector instances using Linux-kernel specific
/// methods, such as cgroups and namespaces.
pub struct SandboxConnectorHandle {
    client: Arc<dyn Connector>,
    socket: PathBuf,
    error_dump: PathBuf,
    read_thread: Option<JoinHandle<()>>,
    pid: Pid,
}

impl SandboxConnectorHandle {
    pub fn still_alive(&self) -> anyhow::Result<i32> {
        if kill(self.pid, None).is_ok() {
            Ok(self.pid.as_raw())
        } else if self.error_dump.is_file() {
            match std::fs::read_to_string(&self.error_dump) {
                Ok(dump) => Err(ErrorMessage { msg: dump }.into()),
                Err(e) => {
                    bail!("Connector process exited, failed to read error dump: {}", e)
                }
            }
        } else {
            bail!("Connector process exited without any error dump!")
        }
    }
}

#[async_trait]
impl Connector for SandboxConnectorHandle {
    async fn new(_name: &str, _prefix: &Path, _outbox: ConnectorOutbox) -> Result<Arc<dyn Connector>, anyhow::Error> {
        bail!("Connector::new() for SandboxConnectorHandle is a stub!")
    }
    async fn init(&self) -> Result<(), anyhow::Error> {
        self.still_alive().context("Before init()".to_string())?;
        let res = Connector::init(&self.client).await;
        self.still_alive().context("After init()".to_string())?;
        res
    }

    async fn version(&self) -> Result<String, anyhow::Error> {
        self.still_alive().context("Before version()".to_string())?;
        let res = Connector::version(&self.client).await;
        self.still_alive().context("After version()".to_string())?;
        res
    }

    async fn filter(&self, addr: &Path) -> Result<FilterResponse, anyhow::Error> {
        self.still_alive().context(format!("Before filter({:?})", addr))?;
        let res = Connector::filter(&self.client, addr).await;
        self.still_alive().context(format!("After filter({:?})", addr))?;
        res
    }

    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>> {
        self.still_alive().context(format!("Before list({:?})", subpath))?;
        let res = Connector::list(&self.client, subpath).await;
        self.still_alive().context(format!("After list({:?})", subpath))?;
        res
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceResponse>, anyhow::Error> {
        self.still_alive().context(format!("Before get({:?})", addr))?;
        let res = Connector::get(&self.client, addr).await;
        self.still_alive().context(format!("After get({:?})", addr))?;
        res
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<PlanResponseElement>, anyhow::Error> {
        self.still_alive().context(format!("Before plan({:?})", addr))?;
        let res = Connector::plan(&self.client, addr, current, desired).await;
        self.still_alive().context(format!("After plan({:?})", addr))?;
        res
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecResponse, anyhow::Error> {
        self.still_alive().context(format!("Before op_exec({:?})", addr))?;
        let res = Connector::op_exec(&self.client, addr, op).await;
        self.still_alive().context(format!("After op_exec({:?})", addr))?;
        res
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyResponse, anyhow::Error> {
        self.still_alive().context(format!("Before addr_virt_to_phy({:?})", addr))?;
        let res = Connector::addr_virt_to_phy(&self.client, addr).await;
        self.still_alive().context(format!("After addr_virt_to_phy({:?})", addr))?;
        res
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        self.still_alive().context(format!("Before addr_phy_to_virt({:?})", addr))?;
        let res = Connector::addr_phy_to_virt(&self.client, addr).await;
        self.still_alive().context(format!("After addr_phy_to_virt({:?})", addr))?;
        res
    }

    async fn get_skeletons(&self) -> Result<Vec<SkeletonResponse>, anyhow::Error> {
        self.still_alive().context("Before get_skeletons()".to_string())?;
        let res = Connector::get_skeletons(&self.client).await;
        self.still_alive().context("After get_skeletons()".to_string())?;
        res
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> Result<Option<GetDocResponse>, anyhow::Error> {
        self.still_alive().context("Before get_docstring()".to_string())?;
        let res = Connector::get_docstring(&self.client, addr, ident).await;
        self.still_alive().context("After get_docstring()".to_string())?;
        res
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        self.still_alive()
            .context(format!("Before eq({}, _, _)", addr.to_string_lossy()))?;
        let res = Connector::eq(&self.client, addr, a, b).await;
        self.still_alive()
            .context(format!("After eq({}, _, _)", addr.to_string_lossy()))?;
        res
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<Option<DiagnosticResponse>, anyhow::Error> {
        self.still_alive()
            .context(format!("Before diag({}, _, _)", addr.to_string_lossy()))?;
        let res = Connector::diag(&self.client, addr, a).await;
        self.still_alive()
            .context(format!("After diag({}, _, _)", addr.to_string_lossy()))?;
        res
    }

    async fn task_exec(
        &self,
        addr: &Path,
        body: Vec<u8>,
        arg: Option<Vec<u8>>,
        state: Option<Vec<u8>>,
    ) -> anyhow::Result<TaskExecResponse> {
        self.still_alive()
            .context(format!("Before task_exec({}, _, _)", addr.to_string_lossy()))?;
        let res = Connector::task_exec(&self.client, addr, body, arg, state).await;
        self.still_alive()
            .context(format!("After unbundle({}, _, _)", addr.to_string_lossy()))?;
        res
    }

    async fn unbundle(&self, addr: &Path, resource: &[u8]) -> Result<Vec<UnbundleResponseElement>, anyhow::Error> {
        self.still_alive()
            .context(format!("Before unbundle({}, _, _)", addr.to_string_lossy()))?;
        let res = Connector::unbundle(&self.client, addr, resource).await;
        self.still_alive()
            .context(format!("After unbundle({}, _, _)", addr.to_string_lossy()))?;
        res
    }
}

pub static SYSINFO: Lazy<Arc<Mutex<sysinfo::System>>> = Lazy::new(|| Arc::new(Mutex::new(sysinfo::System::new())));

#[async_trait]
impl ConnectorHandle for SandboxConnectorHandle {
    async fn status(&self) -> ConnectorHandleStatus {
        match self.still_alive() {
            Ok(pid) => {
                let pid = sysinfo::Pid::from_u32(pid.try_into().unwrap());
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
            Err(_) => ConnectorHandleStatus::Dead,
        }
    }

    async fn kill(&self) -> anyhow::Result<()> {
        kill(self.pid, SIGKILL)?;
        kill(Pid::from_raw(-self.pid.as_raw()), SIGKILL)?;
        killpg(self.pid, SIGKILL)?;
        Ok(())
    }
}

pub fn pipe_fd_to_outbox(fd: &OwnedFd, outbox: &ConnectorOutbox) {
    let mut buf = [0u8; 4096];
    loop {
        match nix::unistd::read(fd, &mut buf) {
            Ok(0) => {
                // 0 means EOF, so the child closed its end or exited. We're done.
                break;
            }
            Ok(n) => {
                // Process `n` bytes from `buf`
                // e.g. log it, send it somewhere, ...
                let n = std::cmp::min(4096, n);

                // Wait, this will corrupt if a utf-8 codepoint spans a 4kb boundary!
                outbox.send(Some(OsString::from_vec(buf[0..n].to_vec()))).unwrap();
            }
            Err(Errno::EAGAIN) => {}
            Err(err) => {
                // Some I/O error occurred
                tracing::error!("Reader error: {}", err);
                break;
            }
        }
    }
}

/// Log the contents of /proc/self/status.
pub fn log_proc_self_status() {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    eprintln!("/proc/self/status:");
    eprintln!("{}", status);
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

/// Use /proc/self/fd/.. to close all open file descriptors
pub fn close_all_extra_fds() -> anyhow::Result<()> {
    let fds = std::fs::read_dir("/proc/self/fd/")?;

    // Each entry will correspond to a file descriptor open by the current process.
    // We want to close everything except 0, 1, 2 (stdin, stdout, stderr)
    //
    for entry in fds {
        let entry = entry?;
        let name = entry.file_name();

        let Some(fd) = name.to_str() else {
            bail!("non-unicode fd path in /proc/self/fd/?");
        };

        let Ok(fd) = fd.parse::<i32>() else {
            bail!("non-integer fd path in /proc/self/fd/?");
        };

        if matches!(fd, 0..=2) {
            continue;
        }

        unsafe {
            libc::close(fd);
        }
    }

    Ok(())
}

#[allow(unused)]
/// This is a utility function for spawning a shell within the sandbox.
/// Unless we redirect stdio, it'll be interactive!
fn exec_debug_shell() -> ! {
    let sh = CString::new("/bin/sh").unwrap();
    let arg0 = CString::new("sh").unwrap();
    let args = &[arg0];

    let env = &[
        CString::new("PATH=/usr/sbin:/usr/bin:/sbin:/bin").unwrap(),
        CString::new("TERM=xterm-256color").unwrap(),
    ];

    execve(&sh, args, env).expect("execve(/bin/sh) failed");
    unreachable!()
}

/// Create a read-only bind mount from `src` to `dst`,
///  and create a writable overlay on top of it in memory.
pub fn bind_mount_with_overlay(root: &Path, src: &Path, dst: &Path) -> anyhow::Result<()> {
    // Layout:
    // overlay_base/
    //   lower/   (bind-mounted src, read-only)
    //   upper/   (writable)
    //   work/    (required by overlayfs)

    let overlay_base = random_overlay_dir(root);

    create_dir_all(&overlay_base)?;

    nix::mount::mount(Some("tmpfs"), &overlay_base, Some("tmpfs"), MsFlags::empty(), None::<&str>)
        .context("mount tmpfs at overlay")?;

    let lower = overlay_base.join("lower");
    let upper = overlay_base.join("upper");
    let work = overlay_base.join("work");

    create_dir_all(&lower)?;
    create_dir_all(&upper)?;
    create_dir_all(&work)?;

    // First, bind-mount src at lower
    nix::mount::mount(
        Some(&src.canonicalize()?),
        &lower,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )
    .context("bind-mount src to lower")?;

    // // ... and remount lower read-only
    // nix::mount::mount(
    //     None::<&Path>,
    //     &lower,
    //     None::<&str>,
    //     MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
    //     None::<&str>,
    // ).context("remount lower read-only")?;

    // Form the overlay with lower/upper/work...
    let opts = format!(
        "lowerdir={},upperdir={},workdir={},userxattr",
        lower.display(),
        upper.display(),
        work.display(),
    );

    // ...and mount it at dst.
    nix::mount::mount(Some("overlay"), dst, Some("overlay"), MsFlags::empty(), Some(opts.as_str()))
        .context("mount overlayfs")?;

    Ok(())
}

#[allow(unreachable_code)]
#[allow(clippy::too_many_arguments)]
pub async fn launch_server_binary_sandboxed(
    spec: &Spec,
    shortname: &str,
    prefix: &Path,
    env: &HashMap<String, String>,
    outbox: ConnectorOutbox,
    keystore: Option<Arc<dyn KeyStore>>,
    root_squashfs: PathBuf,
    repo_path: PathBuf,
) -> anyhow::Result<SandboxConnectorHandle> {
    let mut env = env.clone();

    let socket = random_socket_path();
    let error_dump = random_error_dump_path();

    if let Some(ref keystore) = keystore {
        env = keystore.unseal_env_map(&env)?;
    } else {
        env = passthrough_secrets_from_env(&env)?;
    }

    // We create a pipe (a linked pair of OwnedFds) for stdout and stderr.
    // In essence, the connector, within the sandbox, will write to stdout,
    // and those bytes will be routed to stdout_w.
    // Meanwhile, the host will read from stdout_r and receive those
    // bytes to capture logs from the connector.
    let (stdout_r, _stdout_w) = pipe()?;
    let (stderr_r, _stderr_w) = pipe()?;

    // The cloned child process is started in a new mount namespace,
    //  new cgroup, ipc, user namespaces, etc.
    // However, it inherits mounts not mounted with MS_PRIVATE along
    //  with open file-descriptors from its parent.
    let flags = CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWCGROUP
        | CloneFlags::CLONE_NEWIPC
        | CloneFlags::CLONE_NEWUSER
        | CloneFlags::CLONE_NEWPID
        | CloneFlags::CLONE_NEWUTS;

    let mut pid: Option<Pid> = None;
    const STACK_SIZE: usize = 4 * 1024 * 1024;
    let mut stack = vec![0_u8; STACK_SIZE];

    let mut c_env: Vec<CString> = Vec::new();

    for (key, value) in &env {
        c_env.push(CString::new(format!("{}={}", key, value))?)
    }

    let host_uid = geteuid();
    let host_gid = getegid();

    let keystore = keystore.clone();

    eprintln!("repo_path: {}", repo_path.display());

    unsafe {
        // Here is where we enter the new Linux namespace through clone()!
        let res = nix::sched::clone(
            Box::new(|| {
                {
                    // Right now, we're not running under the host's UID (e.g. 1000). We're running under the overflow UID!
                    // If you ran "whoami", it would report "nobody".

                    // We're going to declare to the kernel that we'd like UID 0 (root) within this namespace
                    //  to correspond to our host UID outside of the namespace.
                    //
                    // Suppose our host_uid is 1000 (the user that initially executed the "autoschematic" binary).
                    // Then, the entry "0 1000 1" defines a mapping:
                    //  - from user 0 within this namespace
                    //  - to user 1000 in the parent namespace
                    //  - of length 1
                    // See user_namespaces(7), section title:  User and group ID mappings: uid_map and gid_map
                    // https://man7.org/linux/man-pages/man7/user_namespaces.7.html

                    std::fs::write("/proc/self/uid_map", format!("0 {} 1", host_uid))
                        .expect("Couldn't write to /proc/self/uid_map!");

                    // We must disable the setgroups(2) syscall before we write to gid_map.
                    std::fs::write("/proc/self/setgroups", "deny").expect("Couldn't write to /proc/self/setgroups!");

                    // // Likewise, declare to the kernel that we'd like GID 0 (root) within this namespace to correspond to our host GID outside of the namespace.
                    std::fs::write("/proc/self/gid_map", format!("0 {} 1", host_gid))
                        .expect("Couldn't write to /proc/self/gid_map!");

                    // sudo make me a sandwich
                    // Set our effective, real, and saved UID to 0 (root).
                    // After this, we're running as root within this user namespace,
                    // and as the host user outside the namespace.
                    // This means we have CAP_SYS_ADMIN and can mount things!
                    setresuid(Uid::from_raw(0), Uid::from_raw(0), Uid::from_raw(0))
                        .expect("Couldn't setresuid to 0 in sandbox");
                    setresgid(Gid::from_raw(0), Gid::from_raw(0), Gid::from_raw(0))
                        .expect("Couldn't setresgid to 0 in sandbox");

                    // Now, we're going to prepare to switch root directories with pivot_root!

                    // From pivot_root(2):
                    //   "new_root must be a path to a mount point, but can't be "/".  A
                    //   path that is not already a mount point can be converted into
                    //   one by bind mounting the path onto itself."
                    nix::mount::mount(
                        Some(&root_squashfs),
                        &root_squashfs,
                        None::<&Path>,
                        MsFlags::MS_BIND,
                        None::<&str>,
                    )
                    .expect("Mounting new_root at itself");

                    // Mount and unseal secrets to /secret/...
                    let secret_mount = root_squashfs.join("secret");
                    // create_dir_all(&secret_mount).expect("Creating secret mount dir");

                    // Mount our repository to operate on at /repo/...
                    let repo_mount = root_squashfs.join("repo");
                    // create_dir_all(&repo_mount).expect("Creating repo mount dir");

                    // Create a target for the pivot_root operation. The old "host" rootfs will be
                    //  mounted here within the mount namespace after pivot_root.
                    // We can't just use any dir, though. From pivot_root(2):
                    // "put_old must be at or underneath new_root; that is, adding some
                    //   nonnegative number of "/.." suffixes to the pathname pointed to
                    //   by put_old must yield the same directory as new_root."
                    let old_root_mount = root_squashfs.join(".old_root");
                    // create_dir_all(&old_root_mount).expect("Creating old root mount dir");

                    // We create an ephemeral read-write tmpfs overlay over the squashfs,
                    // so that many trivial things don't fail due to a read-only filesystem.
                    bind_mount_with_overlay(&root_squashfs, &root_squashfs, &root_squashfs).unwrap();

                    // We also create a separate overlay with the repository state
                    //  that the connector will operate on.
                    // This way, it can modify files in the repo without interfering with other connectors,
                    //  and even git pull/push/commit.
                    bind_mount_with_overlay(&root_squashfs, &repo_path, &repo_mount).unwrap();

                    // Because we're in a new mount namespace, other connectors
                    // can't see this mount or anything in it. We'll create a dedicated tmpfs
                    //  to store secret values that policy allows this connector to read.
                    nix::mount::mount(
                        None::<&PathBuf>,
                        &secret_mount,
                        Some("tmpfs"),
                        nix::mount::MsFlags::empty(),
                        Some("size=64m,mode=0700"),
                    )
                    .expect("Mounting secret tmpfs");

                    if let Some(ref keystore) = keystore {
                        unseal_secrets_to_folder(keystore.clone(), &PathBuf::from(prefix), shortname, &secret_mount)
                            .expect("Failed to unseal secrets to connector mount");
                    }

                    // Autoschematic connectors communicate over UNIX sockets under /tmp/autoschematic/....
                    // This means we need to expose a bind mount for this path so that connectors
                    //  within a sandbox can read and write sockets to communicate with the host!
                    nix::mount::mount(
                        None::<&PathBuf>,
                        &root_squashfs.join("tmp"),
                        Some("tmpfs"),
                        nix::mount::MsFlags::empty(),
                        None::<&str>,
                    )
                    .expect("Mounting /tmp tmpfs");

                    create_dir_all(root_squashfs.join("tmp/autoschematic")).expect("Creating /tmp/autoschematic");

                    nix::mount::mount(
                        Some(Path::new("/tmp/autoschematic")),
                        &root_squashfs.join("tmp/autoschematic"),
                        None::<&Path>,
                        MsFlags::MS_BIND,
                        None::<&str>,
                    )
                    .expect("Bind-mounting /tmp/autoschematic");

                    // Here is some real fuckery. Pivot root is slightly like chroot, except it "swaps" two
                    //  mountpoints. Our old mount entry for / will now be at the path old_root_mount (/.old_root/).
                    pivot_root(&root_squashfs, &old_root_mount).expect("pivoting root");

                    // Now we're really "in" the sandbox filesystem!
                    // However, we can still read everything the host user can read under /.old_root/.

                    nix::mount::mount(None::<&str>, "/proc", Some("proc"), MsFlags::empty(), None::<&str>)
                        .expect("Mount a new procfs at /proc");

                    chdir(Path::new("/")).expect("cd /");

                    // Once we detach the old root, we
                    umount2(Path::new("/.old_root"), MntFlags::MNT_DETACH).unwrap();

                    chdir(Path::new("/repo")).expect("cd /repo");

                    // // Redirect stdout/stderr to our pipes
                    // dup2(&stdout_w, &mut OwnedFd::from_raw_fd(libc::STDOUT_FILENO)).unwrap();
                    // dup2(&stderr_w, &mut OwnedFd::from_raw_fd(libc::STDERR_FILENO)).unwrap();

                    // close_all_extra_fds().unwrap();

                    let spec_command = spec.command().unwrap();
                    let mut binary = spec_command.binary;

                    if !binary.is_file() {
                        binary = which::which(binary).unwrap();
                    }
                    let binary_c = binary.clone().into_c_string().unwrap();

                    let mut c_args: Vec<CString> = Vec::new();

                    c_args.push(binary_c.clone());

                    for arg in spec_command.args {
                        c_args.push(CString::new(arg).unwrap());
                    }

                    c_args.push(shortname.into_c_string().unwrap());
                    c_args.push(prefix.into_c_string().unwrap());
                    c_args.push(socket.clone().into_c_string().unwrap());
                    c_args.push(error_dump.clone().into_c_string().unwrap());

                    eprintln!("execve({:?}, {:?}, {:?})", &binary_c, &c_args, &c_env);

                    // exec_debug_shell();

                    execve(&binary_c, &c_args, &c_env).expect("execve");
                    0
                }
            }),
            &mut stack,
            flags,
            None,
        )?;

        if pid.is_none() {
            pid = Some(res);
        }

        // tracing::debug!("Launched connector {:#?} at PID {:#?}", binary_c, pid);
    }

    let stdout_outbox = outbox.clone();
    let _stdout_thread = tokio::task::spawn_blocking(move || {
        pipe_fd_to_outbox(&stdout_r, &stdout_outbox);
    });

    let stderr_outbox = outbox.clone();
    let _stderr_thread = tokio::task::spawn_blocking(move || {
        pipe_fd_to_outbox(&stderr_r, &stderr_outbox);
    });

    tracing::info!("Launching client at {:?}", socket);

    let client = match spec.protocol() {
        crate::config::Protocol::Tarpc => tarpc_bridge::launch_client(&socket).await?,
        crate::config::Protocol::Grpc => grpc_bridge::launch_client(&socket).await?,
    };

    tracing::info!("Launched client.");

    Ok(SandboxConnectorHandle {
        client,
        socket: socket.to_path_buf(),
        error_dump,
        read_thread: None,
        pid: pid.unwrap(),
    })
}

impl Drop for SandboxConnectorHandle {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.socket) {
            Ok(_) => {}
            Err(e) => tracing::warn!("Couldn't remove socket {:?}: {}", self.socket, e),
        }

        match std::fs::remove_file(&self.error_dump) {
            Ok(_) => {}
            Err(e) => tracing::debug!("Couldn't remove error_dump {:?}: {}", self.error_dump, e),
        }

        if self.read_thread.is_some() {
            // handle.
        }

        tracing::debug!("DROP on SandboxConnectorHandle! Killing {}", self.pid);
        // nix::sys::signal::kill(-self.pid, SIGKILL).unwrap();
        // nix::sys::signal::kill(self.pid, SIGKILL).unwrap();
        let _ = kill(self.pid, SIGKILL).map_err(|e| tracing::info!("kill: {:?}", e));
        let _ = kill(Pid::from_raw(-self.pid.as_raw()), SIGKILL).map_err(|e| tracing::info!("kill: {:?}", e));
        let _ = killpg(self.pid, SIGKILL).map_err(|e| tracing::info!("kill: {:?}", e));
    }
}

pub fn unseal_secrets_to_folder(
    keystore: Arc<dyn KeyStore>,
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

#[allow(dead_code)]
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
