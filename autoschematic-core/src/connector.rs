use std::{
    collections::HashMap,
    ffi::OsString,
    fs::File,
    io::BufReader,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::bail;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use async_trait::async_trait;

use crate::{bundle::UnbundleResponseElement, template::ReadOutput};

pub use crate::diag::DiagnosticResponse;

use crate::util::RON;

/// ConnectorOps output by Connector::plan() may declare a set of output values
/// that they will set or delete on execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputValuePlan {
    Set,
    Delete,
}

pub type OutputValueExec = Option<String>;

pub type OutputMapPlan = HashMap<String, OutputValuePlan>;
pub type OutputMapExec = HashMap<String, OutputValueExec>;

pub type OutputMap = HashMap<String, String>;

#[derive(Serialize, Deserialize)]
pub enum OutputMapFile {
    PointerToVirtual(PathBuf),
    OutputMap(OutputMap),
}

impl OutputMapFile {
    pub fn path(prefix: &Path, addr: &Path) -> PathBuf {
        let mut output = prefix.to_path_buf();

        output.push(".outputs");

        // Join the parent portion of `addr`, if it exists
        if let Some(parent) = addr.parent() {
            // Guard against pathological cases like ".." or "." parents
            // by only pushing normal components
            for comp in parent.components() {
                if let Component::Normal(_) = comp {
                    output.push(comp)
                }
            }
        }

        let mut new_filename = OsString::new();
        if let Some(fname) = addr.file_name() {
            new_filename.push(fname);
        } else {
            // If there's no file name at all, we'll just use ".out.ron"
            // so `new_filename` right now is just "." — that's fine.
            // We'll end up producing something like "./office/east/ec2/us-east-1/.out.ron"
        }
        new_filename.push(".out.ron");

        output.push(new_filename);

        output
    }

    pub fn read(prefix: &Path, addr: &Path) -> anyhow::Result<Option<Self>> {
        let output_path = Self::path(prefix, addr);

        if output_path.is_file() {
            let file = File::open(&output_path)?;
            let reader = BufReader::new(file);

            let output: Self = RON.from_reader(reader)?;

            return Ok(Some(output));
        }

        Ok(None)
    }

    pub fn read_recurse(prefix: &Path, addr: &Path) -> anyhow::Result<Option<Self>> {
        let output_path = Self::path(prefix, addr);

        if output_path.is_file() {
            let file = File::open(&output_path)?;
            let reader = BufReader::new(file);

            let output: Self = RON.from_reader(reader)?;

            match &output {
                OutputMapFile::PointerToVirtual(virt_addr) => {
                    return Self::read_recurse(prefix, virt_addr);
                }
                OutputMapFile::OutputMap(_) => return Ok(Some(output)),
            }
        }

        Ok(None)
    }

    pub fn write(&self, prefix: &Path, addr: &Path) -> anyhow::Result<PathBuf> {
        let output_path = Self::path(prefix, addr);
        let pretty_config = PrettyConfig::default();

        let contents = RON.to_string_pretty(self, pretty_config)?;

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if output_path.exists() {
            std::fs::remove_file(&output_path)?;
        }

        std::fs::write(&output_path, contents)?;

        Ok(output_path)
    }

    pub fn write_recurse(&self, prefix: &Path, addr: &Path) -> anyhow::Result<()> {
        let output_path = Self::path(prefix, addr);

        if output_path.is_file() {
            let contents = std::fs::read_to_string(&output_path)?;

            let output: Self = RON.from_str(&contents)?;

            match &output {
                OutputMapFile::PointerToVirtual(virtual_address) => {
                    return self.write_recurse(prefix, virtual_address);
                }
                OutputMapFile::OutputMap(_) => {
                    if let Some(parent) = output_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&output_path, RON.to_string_pretty(self, PrettyConfig::default())?)?;
                }
            }
        } else {
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output_path, RON.to_string_pretty(self, PrettyConfig::default())?)?;
        }

        Ok(())
    }

    // TODO we should disallow infinite recursive links somehow
    pub fn resolve(prefix: &Path, addr: &Path) -> anyhow::Result<Option<VirtualAddress>> {
        let Some(output) = Self::read(prefix, addr)? else {
            return Ok(None);
        };

        match output {
            OutputMapFile::PointerToVirtual(virtual_address) => Self::resolve(prefix, &virtual_address),
            OutputMapFile::OutputMap(_) => Ok(Some(VirtualAddress(addr.to_path_buf()))),
        }
    }

    pub fn get(prefix: &Path, addr: &Path, key: &str) -> anyhow::Result<Option<String>> {
        let Some(output) = Self::read(prefix, addr)? else {
            return Ok(None);
        };

        match output {
            OutputMapFile::PointerToVirtual(virtual_address) => Self::get(prefix, &virtual_address, key),
            OutputMapFile::OutputMap(map) => Ok(map.get(key).cloned()),
        }
    }

    pub fn apply_output_map(prefix: &Path, addr: &Path, output_map_exec: &OutputMapExec) -> anyhow::Result<Option<PathBuf>> {
        let original = Self::read_recurse(prefix, addr)?.unwrap_or(OutputMapFile::OutputMap(HashMap::new()));

        let OutputMapFile::OutputMap(mut original_map) = original else {
            bail!(
                "apply_output_map({}, {}): resolved to a link file!",
                prefix.display(),
                addr.display()
            );
        };

        for (key, value) in output_map_exec {
            match value {
                Some(value) => {
                    original_map.insert(key.clone(), value.clone());
                }
                None => {
                    original_map.remove(key);
                }
            }
        }

        if original_map.is_empty() {
            Ok(None)
        } else {
            OutputMapFile::OutputMap(original_map).write_recurse(prefix, addr)?;
            Ok(Some(Self::path(prefix, addr)))
        }
    }

    pub fn write_link(prefix: &Path, phy_addr: &Path, virt_addr: &Path) -> anyhow::Result<PathBuf> {
        let output_map = Self::PointerToVirtual(virt_addr.to_path_buf());
        output_map.write(prefix, phy_addr)?;
        Ok(Self::path(prefix, phy_addr))
    }

    pub fn delete(prefix: &Path, addr: &Path) -> anyhow::Result<Option<PathBuf>> {
        let path = Self::path(prefix, addr);
        if path.is_file() {
            std::fs::remove_file(&path)?;
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }
}

pub mod handle;
pub mod spawn;
pub mod task_registry;

// #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Copy, Clone)]
#[bitmask_enum::bitmask(u32)]
#[derive(Serialize, Deserialize)]
pub enum FilterResponse {
    Config,
    Resource,
    Bundle,
    Task,
    Metric,
    None = 0b0,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VirtualAddress(pub PathBuf);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhysicalAddress(pub PathBuf);

#[derive(Debug, Serialize, Deserialize)]
/// GetResourceResponse represents the successful result of Connector.get(addr).
/// Where a resource exists at `addr` that is fetched by the connector,
/// `resource_definition`` will contain the connector's string representation of that
/// resource, and `outputs` will contain the
pub struct GetResourceResponse {
    pub resource_definition: Vec<u8>,
    pub outputs: Option<OutputMap>,
}

impl GetResourceResponse {
    /// Write the contents of this GetResourceResponse to disk. Assumes that the caller has the current
    /// directory set to the repo root.
    /// Returns a Vec of the file paths that were actually written.
    pub async fn write(self, prefix: &Path, phy_addr: &Path, virt_addr: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let mut res = Vec::new();

        let body = self.resource_definition;
        let res_path = prefix.join(virt_addr);

        if let Some(parent) = res_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&res_path, body).await?;

        res.push(res_path);

        if let Some(outputs) = self.outputs
            && !outputs.is_empty()
        {
            let output_map_file = OutputMapFile::OutputMap(outputs);
            res.push(output_map_file.write(prefix, virt_addr)?);

            if virt_addr != phy_addr {
                res.push(OutputMapFile::write_link(prefix, phy_addr, virt_addr)?);
            }
        }

        Ok(res)
    }
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
/// GetDocResponse represents the successful result of Connector.get_doc(ident).
/// This represents the Docstring or other documentation corresponding to
/// structs or enums used in resource bodies.
/// Just like Connector::diag(), it is intended for use with autoschematic-lsp
/// to help users write resource bodies manually.
pub struct GetDocResponse {
    pub markdown: String,
}

impl From<&'static str> for GetDocResponse {
    fn from(value: &'static str) -> Self {
        Self {
            markdown: value.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// PlanResponseElement represents the successful result of `Connector.plan(addr, current, desired)``.
/// Specifically, Connector.plan(...) will return a list of one or more PlanResponseElements representing
/// a sequence of steps to take such that ideally, Connector.get(addr) == desired after executing
/// each of those steps.
pub struct PlanResponseElement {
    pub op_definition: String,
    pub writes_outputs: Vec<String>,
    pub friendly_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// OpExecResponse represents the result of a Connector successfully executing a ConnectorOp.
/// Where a ConnectorOp may, for example, return the ID of a created resource,
/// OpExecResponse may be used to store that ID in `outputs`, where it will be
/// saved and committed as {addr}.output.json file adjacent to the addr at which the.
pub struct OpExecResponse {
    pub outputs: Option<OutputMapExec>,
    pub friendly_message: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
/// TaskExecResponse represents the result of a Connector successfully executing a Task.
pub struct TaskExecResponse {
    /// The next value of `state` with which to call task_exec(...) next time. If None, the task is not executed again.
    pub next_state: Option<Vec<u8>>,
    pub modified_files: Option<Vec<PathBuf>>,
    /// Task files, like Resource files, can have associated outputs. Outputs returned here are merged into the task's
    /// output file.
    pub outputs: Option<HashMap<String, Option<String>>>,
    /// Tasks may also return secret values for the runtime to optionally seal and write to disk if desired
    pub secrets: Option<HashMap<PathBuf, Option<String>>>,
    /// Each task_exec phase can return a friendly human-readable message detailing its state.
    pub friendly_message: Option<String>,
    /// Delay the next task_exec phase until at least `delay_until` seconds after the UNIX epoch
    pub delay_until: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
/// SkeletonResponse represents a template of a resource managed by a Connector.
/// A connector can return multiple skeletons through get_skeletons() in order to
/// provide the user with a set of templates to quickly scaffold new resources, or as
/// examples of the kinds of resources that the user can instantiate and manage through the connector.
pub struct SkeletonResponse {
    pub addr: PathBuf,
    pub body: Vec<u8>,
}

/// ConnectorOutbox is primarily used to transmit logs for tracestores across
/// the remote bridge, i.e. tarpc. The usefulness of this may be reexamined later.
pub type ConnectorOutbox = tokio::sync::broadcast::Sender<Option<String>>;
pub type ConnectorInbox = tokio::sync::broadcast::Receiver<Option<String>>;

#[derive(Debug, Serialize, Deserialize)]
/// VirtToPhyResponse represents the result of Connector::addr_virt_to_phy(addr).
/// Where a connector implementation may map a "virtual" name, to, for instance, route table ID within
/// a VPC, or an EC2 instance ID. This allows resources to be created within a repo and named or laid out
/// ahead of their actual creation, even though their "canonical" address, their instance ID etc,
/// is not known until after creation.
pub enum VirtToPhyResponse {
    /// The resource is defined at a "virtual" address, but its physical address is not populated
    /// because it does not exist yet. For example, an EC2 instance may have been
    /// drafted in the repository, but because it doesn't exist yet, its physical address
    /// (in essence its EC2 instance ID) is undefined.
    NotPresent,
    /// The resource is defined at a "virtual" address, but its physical address is not populated
    /// because it does not exist yet, and in addition, its physical address relies on a
    /// parent resource that also does not exist yet.
    /// For example, a new subnet within a new VPC may have been
    /// drafted in the repository, but because the VPC does not exist yet,
    /// the subnet is "deferred". It cannot even be planned until the parent VPC exists.
    Deferred(Vec<ReadOutput>),
    /// The virtual address resolved successfully to a physical address.
    /// For example, an EC2 instance within a repository exists and resolved to its canonical instance-id-derived address.
    /// E.G: aws/ec2/instances/main_supercomputer_1.ron
    ///  ->  aws/ec2/instances/i-398180832.ron
    Present(PathBuf),
    /// For virtual addresses that have no need to map to physical addresses, this represents that trivial mapping.
    Null(PathBuf),
}

#[async_trait]
#[allow(clippy::new_ret_no_self)]
pub trait Connector: Send + Sync {
    /// Attempt to create a new, uninitialized Connector mounted at `prefix`.
    /// Returns `dyn Connector` to allow implementations to dynamically select Connectors by name.
    /// Should not fail due to invalid config - Connector::init() should handle that.
    async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> Result<Arc<dyn Connector>, anyhow::Error>
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
    /// If `addr` falls within the resource address space of this connector, return `FilterResponse::Resource`.
    /// If `addr` is a configuration file for this connector, return `FilterResponse::Config`.
    /// Otherwise, return `FilterResponse::None`.
    /// filter() is cached by the upstream client, but that cache is reset if
    /// a file is modified for which connector.filter() returns `FilterResponse::Config`.
    /// So, filter() can read config files to, say, have multiple k8s clusters under its management,
    /// or multiple SSH hosts to manage files over sshfs, and thus dynamically determine
    /// whether, say, sshfs/hosts/example.com/var/www/some_file.txt is `Resource` or `None`
    /// (depending on whether example.com is listed in its config file or not),
    /// but if it implements this behaviour, it must declare that config file by returning 'CONFIG' for the config file path.
    async fn filter(&self, addr: &Path) -> Result<FilterResponse, anyhow::Error>;

    /// List all "extant" (E.G., currently existing in AWS, k8s, etc...) object paths, whether they exist in local config or not.
    /// subpath is used to constrain the space of queried results.
    /// The subpath "./" applies no constraint. Connectors may choose interpret subpath in order to
    /// E.G. avoid redundant network queries, but there is no requirement that they do so.
    /// The upstream client will automatically handle more fine-grained filtering.
    /// See [addr_matches_filter] for the implementation of that filtering.
    /// For example, if the subpath is "aws/vpc/us-east-2/vpcs/", the AWS VPC connector might choose to parse
    /// that subpath up to "aws/vpc/us-east-2/" and only run the list queries for resources in the us-east-2 region.
    /// It will still return a superset of the resources specified by the full subpath, but a subset of those
    /// specified by "./" - again, clients will do the more fine-grained filtering themselves after the fact.
    /// Connectors inform clients about how deeply they'll parse `subpath` through the `Connector::subpaths()` function.
    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>>;

    /// Describes how the connector's list() implementation orthogonally subdivides the address space in order to
    /// more efficiently parallelize imports spanning large address spaces.
    /// For example, the AWS VPC connector's list() implementation might
    /// return ["aws/vpc/us-east-1", "aws/vpc/us-east-2", ...]
    /// in order to represent to the client that it can efficiently run parallel list() operations
    /// under those subpaths. Then, the list() implementation must guarantee that it can
    /// correctly parse and limit its querying to each subset of the address space.
    /// The implementation could even be more fine-grained and return, for instance,
    /// ["aws/vpc/us-east-1/vpcs", "aws/vpc/us-east-1/internet_gateways", "aws/vpc/us-east-2/vpcs", "aws/vpc/us-east-2/internet_gateways"]
    /// if it can do even deeper parsing, but this example would likely see diminishing returns.
    async fn subpaths(&self) -> anyhow::Result<Vec<PathBuf>> {
        Ok(vec![PathBuf::from("./")])
    }

    /// Get the current "real" state of the object at `addr`.
    /// For instance, get("aws/vpc/us-east-1/vpcs/vpc-0348895.ron") would query the AWS API for the
    /// current state of the vpc with that ID in the us-east-1 region in the account configured,
    /// and form the human-readable (code) representation as contained in the .ron file.
    /// Note that get() only takes physical addresses - the addr_virt_to_phy is always carried out by the
    /// client ahead of time where needed.
    async fn get(&self, addr: &Path) -> Result<Option<GetResourceResponse>, anyhow::Error>;

    /// Determine how to set current -> desired.
    /// Returns a sequence of Ops that can be executed by op_exec.
    /// This function essentially takes,for a given resource address
    /// the current state as returned by `Connector::get(addr)`, (or none if that resource does not exist),
    /// as well as the desired state (as defined by the state of the file at `./${prefix}/${addr}` on disk).
    /// It will return a set of ConnectorOps which, when passed to op_exec and executed successfully in sequence, should  
    /// result in Connector::get(addr)? == desired .
    /// Note that if desired is None, and current is Some(...), this indicates deleting the remote resource at addr.
    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> Result<Vec<PlanResponseElement>, anyhow::Error>;

    /// Execute a ConnectorOp.
    /// OpExecResponse may include output files, containing, for example,
    ///  the resultant IDs of created resources such as EC2 instances or VPCs.
    /// This will be stored at ./{prefix}/{addr}.out.ron,
    ///  or merged if already present.
    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecResponse, anyhow::Error>;

    /// For resources like VPCs whose ID cannot be known until after creation,
    /// we allow returning the resultant vpc_id in outputs after get() or op_exec().
    /// This allows connectors to translate this mapping and resolve a "virtual" path, with a
    /// user-provided "fake" ID (like a human-readable name), into a physical path, with the actual canonical resource ID.
    /// For example, if we created a VPC with virtual addr "aws/vpc/eu-west-2/vpcs/main.ron",
    /// then after creation, we'd have two output files:
    /// `.outputs/aws/vpc/eu-west-2/vpcs/vpc-038598204.ron` -> PointerToVirtual("aws/vpc/eu-west-2/vpcs/main.ron")
    /// `.outputs/aws/vpc/eu-west-2/vpcs/main.ron` -> OutputMap({vpc_id: "vpc-038598204", ...})
    /// addr_virt_to_phy is where connectors define this mapping: that the physical address can be formed from the
    /// virtual address by reading the output map and substituting certain path components.
    /// Most connectors shouldn't need to override this.
    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyResponse, anyhow::Error> {
        Ok(VirtToPhyResponse::Null(addr.into()))
    }

    /// Reverses the mapping from addr_virt_to_phy. Usually this just means traversing the resource
    /// hierarchy for each parent resource and resolving it with ResourceAddress::phy_to_virt.
    /// See the AWS VpcConnector for an example implementation.
    /// Most connectors shouldn't need to implement this and can just return `addr`.
    /// `None` indicates a failure to resolve the address, such as a dangling output file at a physical address
    /// with a PointerToVirtual entry that no longer exists.
    async fn addr_phy_to_virt(&self, addr: &Path) -> anyhow::Result<Option<PathBuf>> {
        Ok(Some(addr.into()))
    }

    /// To aid development, connectors can provide the user with a set of
    /// "skeleton" resources outlining each type of resource managed by the connector.
    /// Each skeleton resource has an address with `[square_brackets]` for the variable portions,
    /// and the body of the resource should serve as a valid example instance of the resource.
    async fn get_skeletons(&self) -> anyhow::Result<Vec<SkeletonResponse>> {
        Ok(Vec::new())
    }

    /// Connectors may additionally serve docstrings. This is intended to aid development
    /// from an IDE or similar, with a language server hooking into connectors on hover.
    async fn get_docstring(&self, _addr: &Path, _ident: DocIdent) -> anyhow::Result<Option<GetDocResponse>> {
        Ok(None)
    }

    /// Corresponds to an implementation of PartialEq for the underlying resource types
    ///  as parsed by the connector.
    /// This is used in, for example, pull-state, in order to determine if local state needs to be updated
    ///  to match remote state.
    /// The defaul implementation simply compares strings, without serializing or parsing in any way.
    /// addr is ignored in this default case.
    async fn eq(&self, _addr: &Path, a: &[u8], b: &[u8]) -> anyhow::Result<bool> {
        Ok(a == b)
    }

    /// If a resource at `addr` with body `a` fails to parse, connectors may return diagnostics
    /// that outline where the parsing failed with error information.
    /// This is intended to aid development from an IDE or similar, with a language server hooking into connectors.
    async fn diag(&self, _addr: &Path, _a: &[u8]) -> anyhow::Result<Option<DiagnosticResponse>> {
        Ok(None)
    }

    /// Where a Connector or Bundle implementation may define a bundle, with its associated ResourceAddress and Resource formats,
    /// This is where that bundle will be unpacked into one or more resources.
    async fn unbundle(&self, _addr: &Path, _bundle: &[u8]) -> anyhow::Result<Vec<UnbundleResponseElement>> {
        Ok(Vec::new())
    }

    /// Design: TODO: Maybe we'll have task_send_msg(handle, msg) and task_recv_msg(handle) -> Option<msg>?
    /// ...as well as list_task_handles()?
    /// This is an area, like global repo locking, where we ought to be careful about how
    /// we serialize task messages in order to be flexible regarding our shared store over e.g. redis
    /// POST: Ok, now we've hit on a stateless method pattern! This is good. State and task handles can live
    /// in the runtime where they belong. Now, the question is how do we send messages to the runtime from a connector?
    /// This is not strictly speaking task related, but it's worth trying to understand how we can implement,
    /// say, tasks creating or rotating sealed secrets, or creating PRs, etc etc etc...
    ///
    async fn task_exec(
        &self,
        _addr: &Path,
        _body: Vec<u8>,

        // `arg` sets the initial argument for the task. `arg` is set to None after the first execution.
        _arg: Option<Vec<u8>>,
        // The current state of the task as returned by a previous task_exec(...) call.
        // state always starts as None when a task is first executed.
        _state: Option<Vec<u8>>,
    ) -> anyhow::Result<TaskExecResponse> {
        Ok(TaskExecResponse::default())
    }

    /// Design: TODO: How shall we define the GetMetricResponse enum?
    /// Again, how will metrics be stored by the server and queried?
    async fn list_metrics(&self, _addr: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn read_metric(&self, _addr: &Path, _name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn version(&self) -> anyhow::Result<String> {
        Ok(env!("CARGO_PKG_VERSION").to_string())
    }
}

// Helper traits for defining custom internal types in Connector implementations.
// Note that such types are erased by definition at the Connector interface boundary.

/// Resource represents a resource body, either the contents of a file on disk, or
/// a virtual, remote resource as returned by `Connector::get(addr)`.
/// Connectors implement implement and consume their own Resource types. The actual
/// types themselves are erased at the interface between Autoschematic and the Connector implementations
/// it instantiates.
/// For example, Connector::plan(addr, current, desired) takes a &Path, Option<&\[u8\]>, Option<&\[u8\]>,
/// and the connector implementation will parse that as an internal implementation of ResourceAddress, Option<Resource>, Option<Resource>,
/// in order to produce a Vec of structs that implement ConnectorOp, which it will then pass back in serialized form as Vec<String>.
/// Then, Connector::op_exec(addr, connector_op) will similarly parse the raw addr and connector_op into its internal implementations of ResourceAddress and ConnectorOp
/// in order to interpret them and execute an operation.
pub trait Resource: Send + Sync {
    fn to_bytes(&self) -> anyhow::Result<Vec<u8>>;

    fn from_bytes(addr: &impl ResourceAddress, s: &[u8]) -> anyhow::Result<Self>
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
    fn from_path(path: &Path) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn get_output(&self, prefix: &Path, key: &str) -> anyhow::Result<Option<String>> {
        let addr = self.to_path_buf();
        OutputMapFile::get(prefix, &addr, key)
    }

    fn phy_to_virt(&self, prefix: &Path) -> anyhow::Result<Option<Self>> {
        let Some(virt_addr) = OutputMapFile::resolve(prefix, &self.to_path_buf())? else {
            return Ok(None);
        };

        Ok(Some(Self::from_path(&virt_addr.0)?))
    }
}

/// A ConnectorOp represents a single discrete operation that a Connector can execute.
/// Not all ConnectorOps can be truly idempotent, but they should make efforts to be so.
/// ConnectorOps should be as granular as the Connector requires.
/// ConnectorOps are always executed with an associated ResourceAddress
/// through Connector::op_exec(ResourceAddress, ConnectorOp).
pub trait ConnectorOp: Send + Sync + std::fmt::Debug {
    fn to_string(&self) -> anyhow::Result<String>;
    fn from_str(s: &str) -> anyhow::Result<Self>
    where
        Self: Sized;
    /// A human-readable message in plain english explaining what this connector op will do.
    /// You should use the imperative mood, for example:
    /// "Modify tags for IAM role Steve", or
    /// "Delete VPC vpc-923898 in region us-south-5"
    fn friendly_plan_message(&self) -> Option<String> {
        None
    }
    /// A human-readable message in plain english explaining what this connector op has just done.
    /// You should use the past-tense indicative mood, for example:
    /// "Modified tags for IAM role Steve", or
    /// "Deleted VPC vpc-923898 in region us-south-5"
    fn friendly_exec_message(&self) -> Option<String> {
        None
    }
}

#[async_trait]
impl Connector for Arc<dyn Connector> {
    async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> anyhow::Result<Arc<dyn Connector>> {
        return Self::new(name, prefix, outbox).await;
    }

    async fn init(&self) -> anyhow::Result<()> {
        Connector::init(self.as_ref()).await
    }

    async fn version(&self) -> anyhow::Result<String> {
        Connector::version(self.as_ref()).await
    }


    async fn filter(&self, addr: &Path) -> anyhow::Result<FilterResponse> {
        Connector::filter(self.as_ref(), addr).await
    }

    async fn list(&self, subpath: &Path) -> anyhow::Result<Vec<PathBuf>> {
        Connector::list(self.as_ref(), subpath).await
    }

    async fn subpaths(&self) -> anyhow::Result<Vec<PathBuf>> {
        Connector::subpaths(self.as_ref()).await
    }

    async fn get(&self, addr: &Path) -> anyhow::Result<Option<GetResourceResponse>> {
        Connector::get(self.as_ref(), addr).await
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<Vec<u8>>,
        desired: Option<Vec<u8>>,
    ) -> anyhow::Result<Vec<PlanResponseElement>> {
        Connector::plan(self.as_ref(), addr, current, desired).await
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> anyhow::Result<OpExecResponse> {
        Connector::op_exec(self.as_ref(), addr, op).await
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> anyhow::Result<VirtToPhyResponse> {
        Connector::addr_virt_to_phy(self.as_ref(), addr).await
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> anyhow::Result<Option<PathBuf>> {
        Connector::addr_phy_to_virt(self.as_ref(), addr).await
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> anyhow::Result<Option<GetDocResponse>> {
        Connector::get_docstring(self.as_ref(), addr, ident).await
    }

    async fn get_skeletons(&self) -> anyhow::Result<Vec<SkeletonResponse>> {
        Connector::get_skeletons(self.as_ref()).await
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> anyhow::Result<bool> {
        Connector::eq(self.as_ref(), addr, a, b).await
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> anyhow::Result<Option<DiagnosticResponse>> {
        Connector::diag(self.as_ref(), addr, a).await
    }

    async fn unbundle(&self, addr: &Path, bundle: &[u8]) -> anyhow::Result<Vec<UnbundleResponseElement>> {
        Connector::unbundle(self.as_ref(), addr, bundle).await
    }
}
