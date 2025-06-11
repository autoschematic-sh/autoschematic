use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use async_trait::async_trait;

use crate::{diag::DiagnosticOutput, read_outputs::ReadOutput};

pub type OutputMap = HashMap<String, Option<String>>;
pub type OutputMapFile = HashMap<String, String>;

pub mod parse;
pub mod spawn;
pub mod r#type;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Clone)]
pub enum FilterOutput {
    Config,
    Resource,
    None,
}

#[derive(Debug, Serialize, Deserialize)]
/// GetResourceOutput represents the successful result of Connector.get(addr).
/// Where a resource exists at `addr` that is fetched by the connector,
/// `resource_definition`` will contain the connector's string representation of that
/// resource, and `outputs` will contain the
pub struct GetResourceOutput {
    pub resource_definition: Vec<u8>,
    pub outputs: Option<OutputMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// DocIdent represents the target of Connector::GetDocstring().
/// It is used by connector implementations to determine which specific item to return documentation for,
/// whether it be a struct or a field of a struct.
pub enum DocIdent {
    Struct { name: String },
    // Enum { name: String },
    // EnumVariant { parent: String, name: String },
    Field { parent: String, name: String },
}

#[derive(Debug, Serialize, Deserialize)]
/// GetDocOutput represents the successful result of Connector.get_doc(ident).
/// This represents the Docstring or other documentation corresponding to
/// structs or enums used in resource bodies.
/// Just like Connector::diag(), it is intended for use with autoschematic-lsp
/// to help users write resource bodies manually.
pub struct GetDocOutput {
    pub markdown: String,
}

impl From<&'static str> for GetDocOutput {
    fn from(value: &'static str) -> Self {
        Self {
            markdown: value.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// OpPlanOutput represents the successful result of `Connector.plan(addr, current, desired)``.
/// Specifically, Connector.plan(...) will return a list of one or more OpPlanOutputs representing
/// a sequence of steps to take such that:
pub struct OpPlanOutput {
    pub op_definition: String,
    pub writes_outputs: Vec<String>,
    pub friendly_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// OpExecOutput represents the result of a Connector successfully executing a ConnectorOp.
/// Where a ConnectorOp may, for example, return the ID of a created resource,
/// OpExecOutput may be used to store that ID in `outputs`, where it will be
/// saved and committed as {addr}.output.json file adjacent to the addr at which the.
pub struct OpExecOutput {
    pub outputs: Option<HashMap<String, Option<String>>>,
    pub friendly_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
/// SkeletonOutput represents a template of a resource managed by a Connector.
/// A connector can return multiple skeletons through get_skeletons() in order to
/// provide the user with a set of template resources to be used by other tools, or as
/// examples of the kinds of resources that the user can instantiate and manage through the connector.
pub struct SkeletonOutput {
    pub addr: PathBuf,
    pub body: Vec<u8>,
}

pub type ConnectorOutbox = tokio::sync::broadcast::Sender<Option<String>>;
pub type ConnectorInbox = tokio::sync::broadcast::Receiver<Option<String>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListOutput {
    pub addr: PathBuf,

    pub body: Option<Vec<u8>>,
}

pub type ListResultOutbox = tokio::sync::mpsc::Sender<ListOutput>;
pub type ListResultInbox = tokio::sync::mpsc::Receiver<ListOutput>;

#[derive(Debug, Serialize, Deserialize)]
/// VirtToPhyOutput represents the result of Connector::addr_virt_to_phy(addr).
/// Where a connector implementation may map a "virtual" name, to, for instance, route table ID within
/// a VPC, or an EC2 instance ID. This allows resources to be created within a repo and named or laid out
/// ahead of their actual creation, even though their "canonical" address, their instance ID etc,
/// is not known until after creation.
pub enum VirtToPhyOutput {
    /// The resource is defined at a "virtual" address, but its physical address is not populated
    /// because it does not exist yet. For example, an EC2 instance may have been
    /// drafted in the repository, but because it doesn't exist yet, its physical address
    /// (in essence its EC2 instance ID) is undefined.
    NotPresent,
    // Partial(PathBuf),
    /// The resource is defined at a "virtual" address, but its physical address is not populated
    /// because it does not exist yet, and in addition, its physical address relies on a
    /// parent resource that also does not exist yet.
    /// For example, a new subnet within a new VPC may have been
    /// drafted in the repository, but because the VPC does not exist yet,
    /// the subnet is "deferred".
    Deferred(Vec<ReadOutput>),
    /// The virtual address resolved successfully to a physical address.
    /// For example, an EC2 instance within a repository exists and resolved to its canonical instance-id-derived address.
    Present(PathBuf),
}

#[async_trait]
pub trait Connector: Send + Sync {
    /// Attempt to create a new, uninitialized Connector mounted at `prefix`.
    /// Returns `dyn Connector` to allow implementations to dynamically select Connectors by name.
    /// Should not fail due to invalid config - Connector::init() should handle that.
    async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> Result<Box<dyn Connector>, anyhow::Error>
    where
        Self: Sized;

    /// Attempt to initialize, or reinitialize, a Connector.
    /// This will read from environment variables, config files, etc and
    /// may fail on invalid configuration.
    /// Methods like Connector::eq() and Connector::diag() may
    /// still be possible even when uninitialized or when
    /// `Connector::init()` has failed.
    async fn init(&self) -> Result<(), anyhow::Error>;

    /// For a given file within a prefix, this function determines if that file
    /// corresponds to a resource managed by this connector, a configuration file
    /// controlling this connector, or neither.
    /// In essence, this decides on the subset of the address space that the connector
    /// will manage, where "address space" is the nested hierarchy of files.
    /// If `addr` falls within the resource address space of this connector, return `FilterOutput::Resource`.
    /// If `addr` is a configuration file for this connector, return `FilterOutput::Config`.
    /// Otherwise, return `FilterOutput::None`.
    async fn filter(&self, addr: &Path) -> Result<FilterOutput, anyhow::Error>;

    /// List all "extant" (E.G., currently existing in AWS) object paths, whether they exist in local config or not
    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>>;

    /// Get the current "real" state of the object at `addr`
    async fn get(&self, addr: &Path) -> Result<Option<GetResourceOutput>, anyhow::Error>;

    /// Determine how to set current -> desired
    /// Returns a sequence of Ops that can be executed by op_exec.
    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error>;

    /// Execute an Op.
    /// OpExecOutput may include output files, containing, for example,
    ///  the resultant IDs of created resources such as EC2 instances or VPCs.
    /// This will be stored at ./{prefix}/{addr}.out.json,
    ///  or merged if already present.
    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecOutput, anyhow::Error>;

    /// For resources like VPCs whose ID cannot be known until after creation,
    /// we allow returning the resultant vpc_id in outputs after get() or op_exec().
    /// This allows connectors to translate this mapping and resolve a "virtual" path, with a
    /// user-provided "fake" ID, into a physical path, with the actual canonical resource ID.
    /// Connectors `addr_virt_to_phy`
    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyOutput, anyhow::Error> {
        Ok(VirtToPhyOutput::Present(addr.into()))
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        Ok(Some(addr.into()))
    }

    /// To aid development, connectors can provide the user with a set of
    /// "skeleton" resources outlining each type of resource managed by the connector.
    /// Each skeleton resource has an address with `[square_brackets]` for the variable portions,
    /// and the body of the resource should serve as a valid example instance of the resource.
    async fn get_skeletons(&self) -> Result<Vec<SkeletonOutput>, anyhow::Error> {
        Ok(Vec::new())
    }

    /// Connectors may additionally serve docstrings. This is intended to aid development
    /// from an IDE or similar, with a language server hooking into connectors on hover.
    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> anyhow::Result<Option<GetDocOutput>> {
        Ok(None)
    }

    /// Corresponds to an implementation of PartialEq for the underlying resource types
    ///  as parsed by the connector.
    /// This is used in, for example, pull-state, in order to determine if local state needs to be updated
    ///  to match remote state.
    /// The defaul implementation simply compares strings, without serializing or parsing in any way.
    /// addr is ignored in this default case.
    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error>;

    /// If a resource at `addr` with body `a` fails to parse, connectors may return diagnostics
    /// that outline where the parsing failed with error information.
    /// This is intended to aid development from an IDE or similar, with a language server hooking into connectors.
    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<DiagnosticOutput, anyhow::Error>;
}

// Helper traits for defining custom internal types in Connector implementations.
// Note that such types are erased by definition at the Connector interface boundary.

/// Resource represents a resource body, either the contents of a file on disk, or
/// a virtual, remote resource as returned by `Connector::get(addr)`.
pub trait Resource: Send + Sync {
    fn to_bytes(&self) -> Result<Vec<u8>, anyhow::Error>;

    fn from_bytes(addr: &impl ResourceAddress, s: &[u8]) -> Result<Self, anyhow::Error>
    where
        Self: Sized;
}

/// A ResourceAddress represents a unique identifier addressing a single resource
/// unambiguously within a prefix. A given ResourceAddress should have a static,
/// bidirectional mapping to a relative file path.
/// `{prefix}/{addr.to_path_buf()}` with a given connector configuration should therefore
/// serve to globally and uniquely identify a particular resource of a particular type.
pub trait ResourceAddress: Send + Sync + Clone + std::fmt::Debug {
    /// Produce the path in the repository corresponding to this resource address
    fn to_path_buf(&self) -> PathBuf;

    /// Produce the resource address corresponding to a path
    fn from_path(path: &Path) -> Result<Self, anyhow::Error>
    where
        Self: Sized;
}

/// A ConnectorOp represents a
pub trait ConnectorOp: Send + Sync + std::fmt::Debug {
    fn to_string(&self) -> Result<String, anyhow::Error>;
    fn from_str(s: &str) -> Result<Self, anyhow::Error>
    where
        Self: Sized;
}

#[async_trait]
impl Connector for Box<dyn Connector> {
    async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> Result<Box<dyn Connector>, anyhow::Error> {
        return Self::new(name, prefix, outbox).await;
    }

    async fn init(&self) -> Result<(), anyhow::Error> {
        Connector::init(self.as_ref()).await
    }

    async fn filter(&self, addr: &Path) -> Result<FilterOutput, anyhow::Error> {
        Connector::filter(self.as_ref(), addr).await
    }

    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>> {
        Connector::list(self.as_ref(), subpath).await
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceOutput>, anyhow::Error> {
        Connector::get(self.as_ref(), addr).await
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error> {
        Connector::plan(self.as_ref(), addr, current, desired).await
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecOutput, anyhow::Error> {
        Connector::op_exec(self.as_ref(), addr, op).await
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyOutput, anyhow::Error> {
        Connector::addr_virt_to_phy(self.as_ref(), addr).await
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        Connector::addr_phy_to_virt(self.as_ref(), addr).await
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> Result<Option<GetDocOutput>, anyhow::Error> {
        Connector::get_docstring(self.as_ref(), addr, ident).await
    }

    async fn get_skeletons(&self) -> Result<Vec<SkeletonOutput>, anyhow::Error> {
        Connector::get_skeletons(self.as_ref()).await
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        Connector::eq(self.as_ref(), addr, a, b).await
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<DiagnosticOutput, anyhow::Error> {
        Connector::diag(self.as_ref(), addr, a).await
    }
}
