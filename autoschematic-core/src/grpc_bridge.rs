use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use tokio::{net::UnixListener, sync::Mutex};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{
    Request, Response, Status,
    transport::{Endpoint, Server},
};

use tracing_subscriber::EnvFilter;

use crate::{
    bundle::UnbundleResponseElement,
    connector::{ConnectorOutbox, spawn::wait_for_socket},
    diag::DiagnosticResponse,
};

use crate::connector;
use crate::connector::Connector;

pub mod proto {
    include!("./grpc_generated/connector.rs");
}
use proto::{
    connector_client::ConnectorClient as GrpcClient,
    connector_server::{Connector as GrpcConnector, ConnectorServer},
    *,
};

#[derive(Clone)]
pub struct GrpcConnectorServer {
    inner: Arc<Mutex<Arc<dyn Connector>>>,
}

#[async_trait]
impl GrpcConnector for GrpcConnectorServer {
    async fn init(&self, _req: Request<Empty>) -> Result<Response<Empty>, Status> {
        Connector::init(&*self.inner.lock().await)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Empty {}))
    }

    async fn filter(&self, req: Request<FilterRequest>) -> Result<Response<proto::FilterResponse>, Status> {
        let addr = PathBuf::from(req.into_inner().addr);
        let out = Connector::filter(&*self.inner.lock().await, &addr)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let filt = match out {
            connector::FilterResponse::Config => proto::FilterResponseType::Config,
            connector::FilterResponse::Resource => proto::FilterResponseType::Resource,
            connector::FilterResponse::Bundle => proto::FilterResponseType::Bundle,
            connector::FilterResponse::Task => proto::FilterResponseType::Task,
            connector::FilterResponse::None => proto::FilterResponseType::None,
        };
        Ok(Response::new(proto::FilterResponse { filter: filt as i32 }))
    }

    async fn list(&self, req: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        let sub = PathBuf::from(req.into_inner().subpath);
        let addrs = Connector::list(&*self.inner.lock().await, &sub)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ListResponse {
            addrs: addrs.into_iter().map(|p| p.to_string_lossy().into()).collect(),
        }))
    }

    async fn subpaths(&self, _req: Request<Empty>) -> Result<Response<SubpathsResponse>, Status> {
        let paths = Connector::subpaths(&*self.inner.lock().await)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(SubpathsResponse {
            subpaths: paths.into_iter().map(|p| p.to_string_lossy().into()).collect(),
        }))
    }

    async fn get(&self, req: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let addr = PathBuf::from(req.into_inner().addr);
        if let Some(resp) = Connector::get(&*self.inner.lock().await, &addr)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        {
            Ok(Response::new(GetResponse {
                exists: true,
                resource_definition: resp.resource_definition,
                outputs: resp.outputs.unwrap_or_default(),
            }))
        } else {
            Ok(Response::new(GetResponse {
                exists: false,
                resource_definition: vec![],
                outputs: std::collections::HashMap::new(),
            }))
        }
    }

    async fn plan(&self, req: Request<PlanRequest>) -> Result<Response<PlanResponse>, Status> {
        let req = req.into_inner();
        let addr = PathBuf::from(req.addr);
        let current = if req.current.is_empty() { None } else { Some(req.current) };
        let desired = if req.desired.is_empty() { None } else { Some(req.desired) };
        let ops = Connector::plan(&*self.inner.lock().await, &addr, current, desired)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let ops_proto = ops
            .into_iter()
            .map(|op| proto::PlanResponseElement {
                op_definition: op.op_definition,
                writes_outputs: op.writes_outputs,
                friendly_message: op.friendly_message.unwrap_or_default(),
            })
            .collect();
        Ok(Response::new(PlanResponse { ops: ops_proto }))
    }

    async fn op_exec(&self, req: Request<OpExecRequest>) -> Result<Response<OpExecResponse>, Status> {
        let r = req.into_inner();
        let addr = PathBuf::from(r.addr);
        let out = Connector::op_exec(&*self.inner.lock().await, &addr, &r.op)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let mut map = std::collections::HashMap::new();
        if let Some(outputs) = out.outputs {
            map = outputs.into_iter().filter_map(|(k, v)| v.map(|s| (k, s))).collect();
        }
        Ok(Response::new(OpExecResponse {
            outputs: map,
            friendly_message: out.friendly_message.unwrap_or_default(),
        }))
    }

    async fn addr_virt_to_phy(&self, req: Request<AddrVirtToPhyRequest>) -> Result<Response<AddrVirtToPhyResponse>, Status> {
        let addr = PathBuf::from(req.into_inner().addr);
        let out = Connector::addr_virt_to_phy(&*self.inner.lock().await, &addr)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let mut msg = AddrVirtToPhyResponse::default();
        use addr_virt_to_phy_response::Result as R;
        match out {
            connector::VirtToPhyResponse::NotPresent => msg.result = Some(R::NotPresent(Empty {})),
            connector::VirtToPhyResponse::Deferred(reads) => {
                let reads_proto = reads
                    .into_iter()
                    .map(|r| ReadOutput {
                        addr: r.addr.to_string_lossy().into(),
                        key: r.key,
                    })
                    .collect();
                msg.result = Some(R::Deferred(Deferred { reads: reads_proto }));
            }
            connector::VirtToPhyResponse::Present(p) => {
                msg.result = Some(R::Present(proto::Path {
                    path: p.to_string_lossy().into(),
                }));
            }
            connector::VirtToPhyResponse::Null(p) => {
                msg.result = Some(R::Null(proto::Path {
                    path: p.to_string_lossy().into(),
                }));
            }
        }
        Ok(Response::new(msg))
    }

    async fn addr_phy_to_virt(&self, req: Request<AddrPhyToVirtRequest>) -> Result<Response<AddrPhyToVirtResponse>, Status> {
        let addr = PathBuf::from(req.into_inner().addr);
        let opt = Connector::addr_phy_to_virt(&*self.inner.lock().await, &addr)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        if let Some(virt) = opt {
            Ok(Response::new(AddrPhyToVirtResponse {
                has_virt: true,
                virt_addr: virt.to_string_lossy().into(),
            }))
        } else {
            Ok(Response::new(AddrPhyToVirtResponse {
                has_virt: false,
                virt_addr: String::new(),
            }))
        }
    }

    async fn get_skeletons(&self, _req: Request<Empty>) -> Result<Response<GetSkeletonsResponse>, Status> {
        let list = Connector::get_skeletons(&*self.inner.lock().await)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let skels = list
            .into_iter()
            .map(|s| proto::Skeleton {
                addr: s.addr.to_string_lossy().into(),
                body: s.body,
            })
            .collect();
        Ok(Response::new(GetSkeletonsResponse { skeletons: skels }))
    }

    async fn get_docstring(&self, req: Request<GetDocRequest>) -> Result<Response<GetDocResponse>, Status> {
        let r = req.into_inner();
        let addr = PathBuf::from(r.addr);
        let Some(proto_ident) = r.ident.and_then(|id| id.ident) else {
            return Err(Status::invalid_argument("no ident"));
        };

        let ident = match proto_ident {
            doc_ident::Ident::Struct(s) => connector::DocIdent::Struct { name: s.name },
            doc_ident::Ident::Field(f) => connector::DocIdent::Field {
                parent: f.parent,
                name: f.name,
            },
        };

        if let Some(resp) = Connector::get_docstring(&*self.inner.lock().await, &addr, ident)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        {
            Ok(Response::new(GetDocResponse {
                has_doc: true,
                markdown: resp.markdown,
            }))
        } else {
            Ok(Response::new(GetDocResponse {
                has_doc: false,
                markdown: String::new(),
            }))
        }
    }

    async fn eq(&self, req: Request<EqRequest>) -> Result<Response<EqResponse>, Status> {
        let r = req.into_inner();
        let addr = PathBuf::from(r.addr);
        let equal = Connector::eq(&*self.inner.lock().await, &addr, &r.a, &r.b)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(EqResponse { equal }))
    }

    async fn diag(&self, req: Request<DiagRequest>) -> Result<Response<DiagResponse>, Status> {
        let r = req.into_inner();
        let addr = PathBuf::from(r.addr);

        if let Some(resp) = Connector::diag(&*self.inner.lock().await, &addr, &r.a)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        {
            Ok(Response::new(DiagResponse {
                diagnostics: resp
                    .diagnostics
                    .into_iter()
                    .map(|d| proto::Diagnostic {
                        severity: d.severity.into(),
                        span: Some(proto::DiagnosticSpan {
                            start: Some(proto::DiagnosticPosition {
                                line: d.span.start.line,
                                col: d.span.start.col,
                            }),
                            end: Some(DiagnosticPosition {
                                line: d.span.end.line,
                                col: d.span.end.col,
                            }),
                        }),
                        message: d.message,
                    })
                    .collect(),
            }))
        } else {
            Ok(Response::new(DiagResponse { diagnostics: Vec::new() }))
        }
    }

    async fn unbundle(&self, req: Request<UnbundleRequest>) -> Result<Response<UnbundleResponse>, Status> {
        let r = req.into_inner();
        let addr = PathBuf::from(r.addr);
        let bundles = Connector::unbundle(&*self.inner.lock().await, &addr, &r.bundle)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let out = bundles
            .into_iter()
            .map(|b| proto::UnbundleResponseElement {
                addr: b.addr.to_string_lossy().into(),
                contents: b.contents,
            })
            .collect();
        Ok(Response::new(UnbundleResponse { bundles: out }))
    }
}

/// Launch the gRPC server over a Unix-domain socket
pub async fn launch_server<C: Connector>(name: &str, prefix: &Path, socket: &Path, outbox: ConnectorOutbox) -> Result<()> {
    let conn_impl = C::new(name, prefix, outbox).await.context("Failed to initialize connector")?;
    let svc = ConnectorServer::new(GrpcConnectorServer {
        inner: Arc::new(Mutex::new(conn_impl)),
    });

    let uds = UnixListener::bind(socket).context("bind failed")?;
    let incoming = UnixListenerStream::new(uds);
    Server::builder()
        .add_service(svc)
        .serve_with_incoming(incoming)
        .await
        .context("gRPC server error")?;
    Ok(())
}

#[derive(Clone)]
pub struct GrpcConnectorClient {
    inner: Arc<Mutex<GrpcClient<tonic::transport::Channel>>>,
}

impl GrpcConnectorClient {
    pub async fn connect(socket: &Path) -> Result<Self> {
        let uri = format!("unix://{}", socket.display());
        let channel = Endpoint::try_from(uri)?.connect().await.context("gRPC dial failed")?;
        Ok(Self {
            inner: Arc::new(Mutex::new(GrpcClient::new(channel))),
        })
    }
}

#[async_trait]
impl Connector for GrpcConnectorClient {
    async fn new(_name: &str, _prefix: &Path, _outbox: ConnectorOutbox) -> Result<Arc<dyn Connector>> {
        bail!("GrpcConnectorClient::new() is a stub!");
    }

    async fn init(&self) -> Result<()> {
        self.inner.lock().await.init(Request::new(Empty {})).await?;
        Ok(())
    }

    async fn filter(&self, addr: &Path) -> Result<connector::FilterResponse> {
        let req = FilterRequest {
            addr: addr.to_string_lossy().into(),
        };
        let resp = self.inner.lock().await.filter(Request::new(req)).await?.into_inner();
        Ok(match resp.filter {
            x if x == proto::FilterResponseType::Config as i32 => connector::FilterResponse::Config,
            x if x == proto::FilterResponseType::Resource as i32 => connector::FilterResponse::Resource,
            x if x == proto::FilterResponseType::Bundle as i32 => connector::FilterResponse::Bundle,
            x if x == proto::FilterResponseType::Task as i32 => connector::FilterResponse::Task,
            _ => connector::FilterResponse::None,
        })
    }

    async fn list(&self, subpath: &Path) -> Result<Vec<PathBuf>> {
        let req = ListRequest {
            subpath: subpath.to_string_lossy().into(),
        };
        let resp = self.inner.lock().await.list(Request::new(req)).await?.into_inner();
        Ok(resp.addrs.into_iter().map(PathBuf::from).collect())
    }

    async fn subpaths(&self) -> Result<Vec<PathBuf>> {
        let resp = self.inner.lock().await.subpaths(Request::new(Empty {})).await?.into_inner();
        Ok(resp.subpaths.into_iter().map(PathBuf::from).collect())
    }

    async fn get(&self, addr: &Path) -> Result<Option<connector::GetResourceResponse>> {
        let req = GetRequest {
            addr: addr.to_string_lossy().into(),
        };
        let resp = self.inner.lock().await.get(Request::new(req)).await?.into_inner();
        if !resp.exists {
            return Ok(None);
        }
        let outputs = if resp.outputs.is_empty() { None } else { Some(resp.outputs) };
        Ok(Some(connector::GetResourceResponse {
            resource_definition: resp.resource_definition,
            outputs,
        }))
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<connector::PlanResponseElement>> {
        let req = PlanRequest {
            addr: addr.to_string_lossy().into(),
            current: current.unwrap_or_default(),
            desired: desired.unwrap_or_default(),
        };
        let resp = self.inner.lock().await.plan(Request::new(req)).await?.into_inner();
        Ok(resp
            .ops
            .into_iter()
            .map(|o| connector::PlanResponseElement {
                op_definition: o.op_definition,
                writes_outputs: o.writes_outputs,
                friendly_message: if o.friendly_message.is_empty() {
                    None
                } else {
                    Some(o.friendly_message)
                },
            })
            .collect())
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<connector::OpExecResponse> {
        let req = OpExecRequest {
            addr: addr.to_string_lossy().into(),
            op: op.into(),
        };
        let resp = self.inner.lock().await.op_exec(Request::new(req)).await?.into_inner();
        let outputs = if resp.outputs.is_empty() {
            None
        } else {
            Some(resp.outputs.into_iter().map(|(k, v)| (k, Some(v))).collect())
        };
        let friendly = if resp.friendly_message.is_empty() {
            None
        } else {
            Some(resp.friendly_message)
        };
        Ok(connector::OpExecResponse {
            outputs,
            friendly_message: friendly,
        })
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<connector::VirtToPhyResponse> {
        let req = AddrVirtToPhyRequest {
            addr: addr.to_string_lossy().into(),
        };
        let msg = self
            .inner
            .lock()
            .await
            .addr_virt_to_phy(Request::new(req))
            .await?
            .into_inner();
        use proto::addr_virt_to_phy_response::Result as R;
        match msg.result.ok_or_else(|| anyhow::anyhow!("no result"))? {
            R::NotPresent(_) => Ok(connector::VirtToPhyResponse::NotPresent),
            R::Deferred(d) => Ok(connector::VirtToPhyResponse::Deferred(
                d.reads
                    .into_iter()
                    .map(|r| crate::template::ReadOutput {
                        addr: PathBuf::from(r.addr),
                        key: r.key,
                    })
                    .collect(),
            )),
            R::Present(p) => Ok(connector::VirtToPhyResponse::Present(PathBuf::from(p.path))),
            R::Null(p) => Ok(connector::VirtToPhyResponse::Null(PathBuf::from(p.path))),
        }
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>> {
        let req = AddrPhyToVirtRequest {
            addr: addr.to_string_lossy().into(),
        };
        let resp = self
            .inner
            .lock()
            .await
            .addr_phy_to_virt(Request::new(req))
            .await?
            .into_inner();
        if resp.has_virt {
            Ok(Some(PathBuf::from(resp.virt_addr)))
        } else {
            Ok(None)
        }
    }

    async fn get_skeletons(&self) -> Result<Vec<connector::SkeletonResponse>> {
        let resp = self
            .inner
            .lock()
            .await
            .get_skeletons(Request::new(Empty {}))
            .await?
            .into_inner();
        Ok(resp
            .skeletons
            .into_iter()
            .map(|s| connector::SkeletonResponse {
                addr: PathBuf::from(s.addr),
                body: s.body,
            })
            .collect())
    }

    async fn get_docstring(&self, addr: &Path, ident: connector::DocIdent) -> Result<Option<connector::GetDocResponse>> {
        let ident = match ident {
            connector::DocIdent::Struct { name } => proto::DocIdent {
                ident: Some(doc_ident::Ident::Struct(StructIdent { name })),
            },
            connector::DocIdent::Field { parent, name } => proto::DocIdent {
                ident: Some(doc_ident::Ident::Field(FieldIdent { parent, name })),
            },
        };

        let req = GetDocRequest {
            addr: addr.to_string_lossy().into(),
            ident: Some(ident),
        };

        let resp = self.inner.lock().await.get_docstring(Request::new(req)).await?.into_inner();

        if resp.has_doc {
            Ok(Some(connector::GetDocResponse { markdown: resp.markdown }))
        } else {
            Ok(None)
        }
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool> {
        let req = EqRequest {
            addr: addr.to_string_lossy().into(),
            a: a.to_vec(),
            b: b.to_vec(),
        };
        let resp = self.inner.lock().await.eq(Request::new(req)).await?.into_inner();
        Ok(resp.equal)
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<Option<connector::DiagnosticResponse>> {
        let req = DiagRequest {
            addr: addr.to_string_lossy().into(),
            a: a.to_vec(),
        };

        let resp = self.inner.lock().await.diag(Request::new(req)).await?.into_inner();

        if resp.diagnostics.is_empty() {
            Ok(None)
        } else {
            Ok(Some(connector::DiagnosticResponse {
                diagnostics: resp
                    .diagnostics
                    .into_iter()
                    .map(|d| crate::diag::Diagnostic {
                        severity: u8::try_from(d.severity).unwrap_or(1u8),
                        span: crate::diag::DiagnosticSpan {
                            start: crate::diag::DiagnosticPosition {
                                line: d.span.unwrap().start.unwrap().line,
                                col: d.span.unwrap().start.unwrap().col,
                            },
                            end: crate::diag::DiagnosticPosition {
                                line: d.span.unwrap().end.unwrap().line,
                                col: d.span.unwrap().end.unwrap().col,
                            },
                        },
                        message: d.message,
                    })
                    .collect(),
            }))
        }

        // let diag_out = msg
        //     .diagnostics
        //     .unwrap_or_default()
        //     .diagnostics
        //     .into_iter()
        //     .map(|d| crate::diag::Diagnostic {
        //         severity: u8::try_from(d.severity).unwrap_or(1u8),
        //         span: crate::diag::DiagnosticSpan {
        //             start: crate::diag::DiagnosticPosition {
        //                 line: d.span.unwrap().start.unwrap().line,
        //                 col: d.span.unwrap().start.unwrap().col,
        //             },
        //             end: crate::diag::DiagnosticPosition {
        //                 line: d.span.unwrap().end.unwrap().line,
        //                 col: d.span.unwrap().end.unwrap().col,
        //             },
        //         },
        //         message: d.message,
        //     })
        //     .collect();
        // Ok(connector::DiagnosticResponse { diagnostics: diag_out })
    }

    async fn unbundle(&self, addr: &Path, bundle: &[u8]) -> Result<Vec<UnbundleResponseElement>> {
        let req = UnbundleRequest {
            addr: addr.to_string_lossy().into(),
            bundle: bundle.to_vec(),
        };
        let resp = self.inner.lock().await.unbundle(Request::new(req)).await?.into_inner();
        Ok(resp
            .bundles
            .into_iter()
            .map(|b| UnbundleResponseElement {
                addr: PathBuf::from(b.addr),
                contents: b.contents,
            })
            .collect())
    }
}

pub async fn launch_client(socket: &Path) -> Result<Arc<dyn Connector>, anyhow::Error> {
    tracing::info!("waiting for  socket...");
    wait_for_socket(socket, Duration::from_secs(30)).await?;
    tracing::info!("Got socket...");

    let connector_client = GrpcConnectorClient::connect(socket).await?;

    Ok(Arc::new(connector_client) as Arc<dyn Connector>)
}

pub async fn grpc_connector_main<T: Connector>() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_thread_ids(false)
        // .with_ansi(false)
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
        launch_server::<T>(&name, &prefix, &socket, outbox).await
    }) {
        Ok(res) => match res.await {
            Ok(_) => {
                tracing::error!("launch_server returned for some reason?");
                Ok(())
            }
            Err(e) => {
                std::fs::write(error_dump, format!("{e:?}")).expect("Failed to write error dump!");
                tracing::error!("launch_server threw an error: {:?}", e);
                Err(e)
            }
        },
        Err(e) => {
            std::fs::write(error_dump, format!("{e:?}")).expect("Failed to write error dump!");
            tracing::error!("launch_server panicked: {:?}", e);
            bail!("launch_server panicked: {:?}", e);
        }
    }
}
