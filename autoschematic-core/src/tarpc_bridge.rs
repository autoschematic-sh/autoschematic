use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use futures::{StreamExt, lock::Mutex};

use anyhow::{Context, bail};
use async_trait::async_trait;
use tarpc::{
    client::Config,
    serde_transport::{self},
    server::{BaseChannel, Channel},
    tokio_serde::formats::Bincode,
    tokio_util::codec::LengthDelimitedCodec,
};
// use tarpc::tokio_serde::formats::Bincode;
use tokio::net::{UnixListener, UnixStream};
use tracing_subscriber::EnvFilter;

use crate::{
    bundle::BundleOutput,
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterOutput, GetDocOutput, GetResourceOutput, OpExecOutput, OpPlanOutput,
        SkeletonOutput, VirtToPhyOutput,
    },
    diag::DiagnosticOutput,
    error::ErrorMessage,
};

#[tarpc::service]
pub trait TarpcConnector {
    async fn init() -> Result<(), ErrorMessage>;

    async fn filter(addr: PathBuf) -> Result<FilterOutput, ErrorMessage>;

    async fn list(subpath: PathBuf) -> Result<Vec<PathBuf>, ErrorMessage>;

    async fn get(addr: PathBuf) -> Result<Option<GetResourceOutput>, ErrorMessage>;

    async fn plan(addr: PathBuf, current: Option<Vec<u8>>, desired: Option<Vec<u8>>)
    -> Result<Vec<OpPlanOutput>, ErrorMessage>;

    async fn op_exec(addr: PathBuf, op: String) -> Result<OpExecOutput, ErrorMessage>;
    async fn addr_virt_to_phy(addr: PathBuf) -> Result<VirtToPhyOutput, ErrorMessage>;
    async fn addr_phy_to_virt(addr: PathBuf) -> Result<Option<PathBuf>, ErrorMessage>;
    async fn get_skeletons() -> Result<Vec<SkeletonOutput>, ErrorMessage>;
    async fn get_docstring(addr: PathBuf, ident: DocIdent) -> Result<Option<GetDocOutput>, ErrorMessage>;
    async fn eq(addr: PathBuf, a: Vec<u8>, b: Vec<u8>) -> Result<bool, ErrorMessage>;
    async fn diag(addr: PathBuf, a: Vec<u8>) -> Result<DiagnosticOutput, ErrorMessage>;
    async fn unbundle(addr: PathBuf, a: Vec<u8>) -> Result<Vec<BundleOutput>, ErrorMessage>;
}

#[derive(Clone)]
pub struct ConnectorServer {
    connector: Arc<Mutex<Box<dyn Connector>>>,
}

impl TarpcConnector for ConnectorServer {
    async fn init(self, _context: ::tarpc::context::Context) -> Result<(), ErrorMessage> {
        Ok(Connector::init(&*self.connector.lock().await).await?)
    }

    async fn filter(self, _context: ::tarpc::context::Context, addr: PathBuf) -> Result<FilterOutput, ErrorMessage> {
        Ok(Connector::filter(&*self.connector.lock().await, &addr).await?)
    }

    async fn list(self, _context: ::tarpc::context::Context, subpath: PathBuf) -> Result<Vec<PathBuf>, ErrorMessage> {
        let res = Connector::list(&*self.connector.lock().await, &subpath).await;
        Ok(res?)
    }

    async fn get(self, _context: ::tarpc::context::Context, addr: PathBuf) -> Result<Option<GetResourceOutput>, ErrorMessage> {
        Ok(Connector::get(&*self.connector.lock().await, &addr).await?)
    }

    async fn plan(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<OpPlanOutput>, ErrorMessage> {
        Ok(Connector::plan(&*self.connector.lock().await, &addr, current, desired).await?)
    }

    async fn op_exec(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
        op: String,
    ) -> Result<OpExecOutput, ErrorMessage> {
        Ok(Connector::op_exec(&*self.connector.lock().await, &addr, &op).await?)
    }

    async fn addr_virt_to_phy(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
    ) -> Result<VirtToPhyOutput, ErrorMessage> {
        Ok(Connector::addr_virt_to_phy(&*self.connector.lock().await, &addr).await?)
    }

    async fn addr_phy_to_virt(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
    ) -> Result<Option<PathBuf>, ErrorMessage> {
        Ok(Connector::addr_phy_to_virt(&*self.connector.lock().await, &addr).await?)
    }

    async fn get_skeletons(self, _context: ::tarpc::context::Context) -> Result<Vec<SkeletonOutput>, ErrorMessage> {
        Ok(Connector::get_skeletons(&*self.connector.lock().await).await?)
    }

    async fn get_docstring(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
        ident: DocIdent,
    ) -> Result<Option<GetDocOutput>, ErrorMessage> {
        Ok(Connector::get_docstring(&*self.connector.lock().await, &addr, ident).await?)
    }

    async fn eq(self, _context: tarpc::context::Context, addr: PathBuf, a: Vec<u8>, b: Vec<u8>) -> Result<bool, ErrorMessage> {
        Ok(Connector::eq(&*self.connector.lock().await, &addr, &a, &b).await?)
    }

    async fn diag(
        self,
        _context: tarpc::context::Context,
        addr: PathBuf,
        a: Vec<u8>,
    ) -> Result<DiagnosticOutput, ErrorMessage> {
        Ok(Connector::diag(&*self.connector.lock().await, &addr, &a).await?)
    }

    async fn unbundle(
        self,
        _context: tarpc::context::Context,
        addr: PathBuf,
        resource: Vec<u8>,
    ) -> Result<Vec<BundleOutput>, ErrorMessage> {
        Ok(Connector::unbundle(&*self.connector.lock().await, &addr, &resource).await?)
    }
}

impl<C: Connector> TarpcConnector for C {
    async fn init(self, _context: ::tarpc::context::Context) -> Result<(), ErrorMessage> {
        Ok(Connector::init(&self).await?)
    }

    async fn filter(self, _context: ::tarpc::context::Context, addr: PathBuf) -> Result<FilterOutput, ErrorMessage> {
        Ok(Connector::filter(&self, &addr).await?)
    }

    async fn list(self, _context: ::tarpc::context::Context, subpath: PathBuf) -> Result<Vec<PathBuf>, ErrorMessage> {
        Ok(Connector::list(&self, &subpath).await?)
    }

    async fn get(self, _context: ::tarpc::context::Context, addr: PathBuf) -> Result<Option<GetResourceOutput>, ErrorMessage> {
        Ok(Connector::get(&self, &addr).await?)
    }

    async fn plan(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<OpPlanOutput>, ErrorMessage> {
        Ok(Connector::plan(&self, &addr, current, desired).await?)
    }

    async fn op_exec(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
        op: String,
    ) -> Result<OpExecOutput, ErrorMessage> {
        Ok(Connector::op_exec(&self, &addr, &op).await?)
    }

    async fn addr_virt_to_phy(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
    ) -> Result<VirtToPhyOutput, ErrorMessage> {
        Ok(Connector::addr_virt_to_phy(&self, &addr).await?)
    }

    async fn addr_phy_to_virt(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
    ) -> Result<Option<PathBuf>, ErrorMessage> {
        Ok(Connector::addr_phy_to_virt(&self, &addr).await?)
    }

    async fn get_skeletons(self, _context: ::tarpc::context::Context) -> Result<Vec<SkeletonOutput>, ErrorMessage> {
        Ok(Connector::get_skeletons(&self).await?)
    }

    async fn get_docstring(
        self,
        _context: ::tarpc::context::Context,
        addr: PathBuf,
        ident: DocIdent,
    ) -> Result<Option<GetDocOutput>, ErrorMessage> {
        Ok(Connector::get_docstring(&self, &addr, ident).await?)
    }

    async fn eq(self, _context: tarpc::context::Context, addr: PathBuf, a: Vec<u8>, b: Vec<u8>) -> Result<bool, ErrorMessage> {
        Ok(Connector::eq(&self, &addr, &a, &b).await?)
    }

    async fn diag(
        self,
        _context: tarpc::context::Context,
        addr: PathBuf,
        a: Vec<u8>,
    ) -> Result<DiagnosticOutput, ErrorMessage> {
        Ok(Connector::diag(&self, &addr, &a).await?)
    }

    async fn unbundle(
        self,
        _context: tarpc::context::Context,
        addr: PathBuf,
        resource: Vec<u8>,
    ) -> Result<Vec<BundleOutput>, ErrorMessage> {
        Ok(Connector::unbundle(&self, &addr, &resource).await?)
    }
}

fn context_100m_deadline() -> tarpc::context::Context {
    let mut context = tarpc::context::Context::current();
    context.deadline = std::time::Instant::now() + std::time::Duration::from_secs(6000);
    context
}

fn context_10m_deadline() -> tarpc::context::Context {
    let mut context = tarpc::context::Context::current();
    context.deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    context
}

fn context_1m_deadline() -> tarpc::context::Context {
    let mut context = tarpc::context::Context::current();
    context.deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    context
}

#[async_trait]
impl Connector for TarpcConnectorClient {
    async fn new(_name: &str, _prefix: &Path, _outbox: ConnectorOutbox) -> Result<Box<dyn Connector>, anyhow::Error> {
        bail!("TarpcConnectorClient::new() is a stub!")
    }

    async fn init(&self) -> Result<(), anyhow::Error> {
        let res = self.init(context_1m_deadline()).await;
        Ok(res??)
    }

    async fn filter(&self, addr: &Path) -> Result<FilterOutput, anyhow::Error> {
        let res = self.filter(context_1m_deadline(), addr.to_path_buf()).await;
        Ok(res??)
    }

    async fn list(&self, subpath: &Path) -> Result<Vec<PathBuf>, anyhow::Error> {
        let res = self.list(context_100m_deadline(), subpath.to_path_buf()).await;
        Ok(res??)
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceOutput>, anyhow::Error> {
        Ok(self.get(context_10m_deadline(), addr.to_path_buf()).await??)
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error> {
        Ok(self
            .plan(context_10m_deadline(), addr.to_path_buf(), current, desired)
            .await??)
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecOutput, anyhow::Error> {
        Ok(self
            .op_exec(context_1m_deadline(), addr.to_path_buf(), op.to_string())
            .await??)
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyOutput, anyhow::Error> {
        Ok(self.addr_virt_to_phy(context_1m_deadline(), addr.to_path_buf()).await??)
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        Ok(self.addr_phy_to_virt(context_1m_deadline(), addr.to_path_buf()).await??)
    }

    async fn get_skeletons(&self) -> Result<Vec<SkeletonOutput>, anyhow::Error> {
        Ok(self.get_skeletons(context_1m_deadline()).await??)
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> Result<Option<GetDocOutput>, anyhow::Error> {
        Ok(self.get_docstring(context_1m_deadline(), addr.to_path_buf(), ident).await??)
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        Ok(self
            .eq(context_1m_deadline(), addr.to_path_buf(), a.to_owned(), b.to_owned())
            .await??)
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<DiagnosticOutput, anyhow::Error> {
        Ok(self.diag(context_1m_deadline(), addr.to_path_buf(), a.to_owned()).await??)
    }

    async fn unbundle(&self, addr: &Path, resource: &[u8]) -> Result<Vec<BundleOutput>, anyhow::Error> {
        Ok(self
            .unbundle(context_1m_deadline(), addr.to_path_buf(), resource.to_owned())
            .await??)
    }
}

async fn wait_for_socket(socket: &Path, timeout: Duration) -> anyhow::Result<()> {
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

pub async fn launch_client(socket: &Path) -> Result<TarpcConnectorClient, anyhow::Error> {
    tracing::info!("waiting for  socket...");
    wait_for_socket(socket, Duration::from_secs(30)).await?;
    tracing::info!("Got socket...");
    let conn = UnixStream::connect(socket).await?;
    tracing::info!("Connected to socket...");
    let codec_builder = LengthDelimitedCodec::builder();

    let transport = serde_transport::new(codec_builder.new_framed(conn), Bincode::default());

    let connector_client = TarpcConnectorClient::new(Config::default(), transport).spawn();

    Ok(connector_client)
}

pub async fn launch_server<C: Connector>(
    name: &str,
    prefix: &Path,
    socket: &Path,
    outbox: tokio::sync::broadcast::Sender<Option<String>>,
) -> anyhow::Result<()> {
    let connector = C::new(name, prefix, outbox).await.context("Failed to initialize connector")?;

    let server = ConnectorServer {
        connector: Arc::new(Mutex::new(connector)),
    };

    let listener = UnixListener::bind(socket).context("Failed to bind socket")?;
    let codec_builder = LengthDelimitedCodec::builder();

    loop {
        let (conn, _addr) = listener.accept().await.context("Failed to accept connection")?;
        let framed = codec_builder.new_framed(conn);
        let transport = serde_transport::new(framed, Bincode::default());

        let server = server.clone();
        let serve_fn = server.serve();

        let fut = BaseChannel::with_defaults(transport).execute(serve_fn).for_each(|s| async {
            tokio::spawn(s);
        });
        tokio::spawn(fut);
    }
}

pub async fn init_server<C: Connector>(
    name: &str,
    prefix: &Path,
    socket: &Path,
    outbox: tokio::sync::broadcast::Sender<Option<String>>,
) -> anyhow::Result<isize> {
    match launch_server::<C>(name, prefix, socket, outbox).await {
        Ok(()) => {
            tracing::error!("launch exited???");
            Ok(0)
        }
        Err(e) => {
            tracing::error!("Error in launch_server: {}", e);
            Err(e)
        }
    }
}

pub async fn tarpc_connector_main<T: Connector>() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_thread_ids(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .compact()
        .init();

    let args: Vec<String> = std::env::args().collect();

    let name = args[1].clone();
    let prefix = PathBuf::from(&args[2]);
    let socket = PathBuf::from(&args[3]);
    let error_dump = PathBuf::from(&args[4]);

    match std::panic::catch_unwind(async move || {
        let (outbox, _inbox) = tokio::sync::broadcast::channel(64);
        init_server::<T>(&name, &prefix, &socket, outbox).await
    }) {
        Ok(res) => match res.await {
            Ok(_) => {
                tracing::error!("init_server returned for some reason?");
                Ok(())
            }
            Err(e) => {
                std::fs::write(error_dump, format!("{:?}", e)).expect("Failed to write error dump!");
                tracing::error!("init_server threw an error: {:?}", e);
                Err(e)
            }
        },
        Err(e) => {
            std::fs::write(error_dump, format!("{:?}", e)).expect("Failed to write error dump!");
            tracing::error!("init_server panicked: {:?}", e);
            bail!("init_server panicked: {:?}", e);
        }
    }
}
