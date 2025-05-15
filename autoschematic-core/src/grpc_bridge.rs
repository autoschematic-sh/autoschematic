use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::bail;
use async_trait::async_trait;
use tokio::{net::{UnixListener, UnixStream}, sync::broadcast::Sender};
use tonic::{transport::{Channel, Server, Uri}, Request, Response, Status};
use tokio_stream::wrappers::UnixListenerStream;

pub mod grpc_connector;

use crate::{
    connector::{Connector, ConnectorOutbox, GetResourceOutput, OpExecOutput, OpPlanOutput},
    error::ErrorMessage,
};
use crate::grpc_bridge::grpc_connector::{
        grpc_connector_client::GrpcConnectorClient,
        grpc_connector_server::{GrpcConnector, GrpcConnectorServer as TonicGrpcConnectorServer},
        BoolResponse, GetResourceOutput as ProtoGetResourceOutput, GetResourceResponse,
        OpExecOutput as ProtoOpExecOutput, OpExecRequest, OpExecResponse, OpPlanOutput as ProtoOpPlanOutput,
        OpPlanOutputListResponse, OptionalPathResponse, OptionalString, PathListResponse, PathRequest,
        PlanRequest,
    };

/// Converts a core ErrorMessage to a proto ErrorMessage
fn to_proto_error(error: ErrorMessage) -> grpc_connector::ErrorMessage {
    grpc_connector::ErrorMessage {
        has_error: true,
        message: error.to_string(),
    }
}

/// Creates an empty proto ErrorMessage (no error)
fn empty_proto_error() -> grpc_connector::ErrorMessage {
    grpc_connector::ErrorMessage {
        has_error: false,
        message: String::new(),
    }
}

/// Converts an Option<String> to OptionalString proto
fn to_proto_option_string(option: Option<String>) -> OptionalString {
    match option {
        Some(value) => OptionalString {
            has_value: true,
            value,
        },
        None => OptionalString {
            has_value: false,
            value: String::new(),
        },
    }
}

/// Converts a HashMap<String, Option<String>> to a HashMap<String, OptionalString> for proto
fn to_proto_output_map(
    map: Option<HashMap<String, Option<String>>>,
) -> HashMap<String, OptionalString> {
    match map {
        Some(map) => {
            let mut out_map = HashMap::new();
            for (k, v) in map.iter() {
                out_map.insert(k.to_string(), to_proto_option_string(v.clone()));
            }
            
            out_map
        }
        None => HashMap::new(),
    }
}

/// Server struct that implements the GrpcConnector trait for Tonic
pub struct ConnectorServer {
    connector: Arc<Mutex<Box<dyn Connector>>>,
}

#[tonic::async_trait]
impl GrpcConnector for ConnectorServer {
    async fn filter(
        &self,
        request: Request<PathRequest>,
    ) -> Result<Response<BoolResponse>, Status> {
        let path = PathBuf::from(request.into_inner().path);
        
        match Connector::filter(&*self.connector.lock().await, &path).await {
            Ok(result) => Ok(Response::new(BoolResponse {
                result,
                error: Some(empty_proto_error()),
            })),
            Err(err) => Ok(Response::new(BoolResponse {
                result: false,
                error: Some(to_proto_error(err.into())),
            })),
        }
    }

    async fn list(
        &self,
        request: Request<PathRequest>,
    ) -> Result<Response<PathListResponse>, Status> {
        let path = PathBuf::from(request.into_inner().path);
        
        match Connector::list(&*self.connector.lock().await, &path).await {
            Ok(paths) => Ok(Response::new(PathListResponse {
                paths: paths.into_iter().map(|p| p.to_string_lossy().to_string()).collect(),
                error: Some(empty_proto_error()),
            })),
            Err(err) => Ok(Response::new(PathListResponse {
                paths: vec![],
                error: Some(to_proto_error(err.into())),
            })),
        }
    }

    async fn get(
        &self,
        request: Request<PathRequest>,
    ) -> Result<Response<GetResourceResponse>, Status> {
        let path = PathBuf::from(request.into_inner().path);
        
        match Connector::get(&*self.connector.lock().await, &path).await {
            Ok(Some(resource)) => Ok(Response::new(GetResourceResponse {
                has_resource: true,
                resource: Some(ProtoGetResourceOutput {
                    resource_definition: resource.resource_definition,
                    outputs: to_proto_output_map(resource.outputs),
                }),
                error: Some(empty_proto_error()),
            })),
            Ok(None) => Ok(Response::new(GetResourceResponse {
                has_resource: false,
                resource: None,
                error: Some(empty_proto_error()),
            })),
            Err(err) => Ok(Response::new(GetResourceResponse {
                has_resource: false,
                resource: None,
                error: Some(to_proto_error(err.into())),
            })),
        }
    }

    async fn plan(
        &self,
        request: Request<PlanRequest>,
    ) -> Result<Response<OpPlanOutputListResponse>, Status> {
        let request = request.into_inner();
        let path = PathBuf::from(request.path);
        
        let current = if request.has_current {
            Some(request.current)
        } else {
            None
        };
        
        let desired = if request.has_desired {
            Some(request.desired)
        } else {
            None
        };

        match Connector::plan(&*self.connector.lock().await, &path, current, desired).await {
            Ok(ops) => {
                let proto_ops = ops
                    .into_iter()
                    .map(|op| ProtoOpPlanOutput {
                        op_definition: op.op_definition,
                        has_friendly_message: op.friendly_message.is_some(),
                        friendly_message: op.friendly_message.unwrap_or_default(),
                    })
                    .collect();

                Ok(Response::new(OpPlanOutputListResponse {
                    ops: proto_ops,
                    error: Some(empty_proto_error()),
                }))
            }
            Err(err) => Ok(Response::new(OpPlanOutputListResponse {
                ops: vec![],
                error: Some(to_proto_error(err.into())),
            })),
        }
    }

    async fn op_exec(
        &self,
        request: Request<OpExecRequest>,
    ) -> Result<Response<OpExecResponse>, Status> {
        let request = request.into_inner();
        let path = PathBuf::from(request.path);
        
        match Connector::op_exec(&*self.connector.lock().await, &path, &request.op).await {
            Ok(output) => Ok(Response::new(OpExecResponse {
                output: Some(ProtoOpExecOutput {
                    outputs: to_proto_output_map(output.outputs),
                    has_friendly_message: output.friendly_message.is_some(),
                    friendly_message: output.friendly_message.unwrap_or_default(),
                }),
                error: Some(empty_proto_error()),
            })),
            Err(err) => Ok(Response::new(OpExecResponse {
                output: None,
                error: Some(to_proto_error(err.into())),
            })),
        }
    }

    async fn addr_virt_to_phy(
        &self,
        request: Request<PathRequest>,
    ) -> Result<Response<OptionalPathResponse>, Status> {
        let path = PathBuf::from(request.into_inner().path);
        
        match Connector::addr_virt_to_phy(&*self.connector.lock().await, &path).await {
            Ok(Some(path)) => Ok(Response::new(OptionalPathResponse {
                has_path: true,
                path: path.to_string_lossy().to_string(),
                error: Some(empty_proto_error()),
            })),
            Ok(None) => Ok(Response::new(OptionalPathResponse {
                has_path: false,
                path: String::new(),
                error: Some(empty_proto_error()),
            })),
            Err(err) => Ok(Response::new(OptionalPathResponse {
                has_path: false,
                path: String::new(),
                error: Some(to_proto_error(err.into())),
            })),
        }
    }
}

/// Adapter to convert from Option<String> stored in proto format back to Rust Option<String>
fn from_proto_option_string(option: Option<OptionalString>) -> Option<String> {
    match option {
        Some(opt) if opt.has_value => Some(opt.value),
        _ => None,
    }
}

/// Converts a HashMap<String, OptionalString> back to HashMap<String, Option<String>>
fn from_proto_output_map(
    map: HashMap<String, OptionalString>,
) -> Option<HashMap<String, Option<String>>> {
    if map.is_empty() {
        None
    } else {
        Some(
            map.into_iter()
                .map(|(k, v)| {
                    let value = if v.has_value { Some(v.value) } else { None };
                    (k, value)
                })
                .collect(),
        )
    }
}

#[derive(Debug)]
pub struct ConnectorClient {
    client: Arc<Mutex<GrpcConnectorClient<Channel>>>,
}

/// Implementation of Connector trait for GrpcConnectorClient
#[async_trait]
impl Connector for ConnectorClient {
    async fn new(
        _name: &str,
        _prefix: &Path,
        _outbox: ConnectorOutbox,
    ) -> Result<Box<dyn Connector>, anyhow::Error> {
        bail!("GrpcConnectorClient::new() is a stub!")
    }

    async fn filter(&self, addr: &Path) -> Result<bool, anyhow::Error> {
        let request = Request::new(PathRequest {
            path: addr.to_string_lossy().to_string(),
        });
        
        let mut client = self.client.get_mut().await;

        let response = GrpcConnectorClient::<Channel>::filter(self, request).await?;
        let result = response.into_inner();

        if result.error.unwrap().has_error {
            bail!(result.error.unwrap().message);
        }

        Ok(result.result)
    }

    async fn list(&self, subpath: &Path) -> Result<Vec<PathBuf>, anyhow::Error> {
        let request = Request::new(PathRequest {
            path: subpath.to_string_lossy().to_string(),
        });

        let response = self.list(request).await?;
        let result = response.into_inner();

        if result.error.has_error {
            bail!(result.error.message);
        }

        Ok(result
            .paths
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceOutput>, anyhow::Error> {
        let request = Request::new(PathRequest {
            path: addr.to_string_lossy().to_string(),
        });

        let response = self.get(request).await?;
        let result = response.into_inner();

        if result.error.has_error {
            bail!(result.error.message);
        }

        if !result.has_resource || result.resource.is_none() {
            return Ok(None);
        }

        let resource = result.resource.unwrap();
        Ok(Some(GetResourceOutput {
            resource_definition: resource.resource_definition,
            outputs: from_proto_output_map(resource.outputs),
        }))
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<String>,
        desired: Option<String>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error> {
        let request = Request::new(PlanRequest {
            path: addr.to_string_lossy().to_string(),
            has_current: current.is_some(),
            current: current.unwrap_or_default(),
            has_desired: desired.is_some(),
            desired: desired.unwrap_or_default(),
        });

        let response = self.plan(request).await?;
        let result = response.into_inner();

        if result.error.has_error {
            bail!(result.error.message);
        }

        Ok(result
            .ops
            .into_iter()
            .map(|op| OpPlanOutput {
                op_definition: op.op_definition,
                friendly_message: if op.has_friendly_message {
                    Some(op.friendly_message)
                } else {
                    None
                },
            })
            .collect())
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecOutput, anyhow::Error> {
        let request = Request::new(OpExecRequest {
            path: addr.to_string_lossy().to_string(),
            op: op.to_string(),
        });

        let response = self.op_exec(request).await?;
        let result = response.into_inner();

        if result.error.has_error {
            bail!(result.error.message);
        }

        if result.output.is_none() {
            bail!("Received empty output from server");
        }

        let output = result.output.unwrap();
        Ok(OpExecOutput {
            outputs: from_proto_output_map(output.outputs),
            friendly_message: if output.has_friendly_message {
                Some(output.friendly_message)
            } else {
                None
            },
        })
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        let request = Request::new(PathRequest {
            path: addr.to_string_lossy().to_string(),
        });

        let response = self.addr_virt_to_phy(request).await?;
        let result = response.into_inner();

        if result.error.has_error {
            bail!(result.error.message);
        }

        if !result.has_path {
            return Ok(None);
        }

        Ok(Some(PathBuf::from(result.path)))
    }
}

/// Wait for a Unix socket to become available
async fn wait_for_socket(socket: &Path, timeout: Duration) -> anyhow::Result<()> {
    let start_time = std::time::Instant::now();

    loop {
        if std::time::Instant::now() - start_time > timeout {
            bail!("Timed out waiting for socket after {:?}", timeout)
        }
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    Ok(())
}

/// Launch a gRPC client connected to a Unix socket
pub async fn launch_client(socket: &Path) -> Result<GrpcConnectorClient<Channel>, anyhow::Error> {
    tracing::info!("waiting for socket...");
    wait_for_socket(socket, Duration::from_secs(30)).await?;
    tracing::info!("Got socket...");

    // Create a channel using Unix socket
    let channel = Channel::from_static("unix:///dummy")
        .connect_with_connector(tower::service_fn(move |_: Uri| {
            let socket_path = socket.to_path_buf();
            async move {
                UnixStream::connect(socket_path)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        }))
        .await?;

    tracing::info!("Connected to socket...");
    Ok(GrpcConnectorClient::new(channel))
}

/// Launch a gRPC server listening on a Unix socket
pub async fn launch_server<C: Connector>(
    name: &str,
    prefix: &Path,
    socket: &Path,
    outbox: Sender<Option<String>>,
) -> anyhow::Result<()> {
    // Create connector
    let connector = C::new(name, prefix, outbox).await?;

    // Create server
    let server = ConnectorServer {
        connector: Arc::new(Mutex::new(connector)),
    };

    // Ensure socket directory exists
    if let Some(parent) = socket.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Remove socket if it already exists
    if socket.exists() {
        std::fs::remove_file(socket)?;
    }

    // Bind to Unix socket
    let uds = UnixListener::bind(socket)?;
    let uds_stream = UnixListenerStream::new(uds);

    // Start the server
    tracing::info!("Starting gRPC server on Unix socket: {:?}", socket);
    Server::builder()
        .add_service(TonicGrpcConnectorServer::new(server))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}

/// Initialize a gRPC server in a new runtime
pub fn init_server<C: Connector>(
    name: &str,
    prefix: &Path,
    socket: &Path,
    outbox: Sender<Option<String>>,
) -> anyhow::Result<isize> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            match launch_server::<C>(name, prefix, socket, outbox).await {
                Ok(()) => {
                    tracing::error!("launch exited???");
                }
                Err(e) => {
                    tracing::error!("Error in launch_server: {}", e)
                }
            }

            tracing::error!("launch exited???");
        });
    tracing::error!("launch exited???");
    Ok(0)
}
