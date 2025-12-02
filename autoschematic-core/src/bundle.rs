use std::{
    collections::HashMap,
    ffi::OsString,
    fs::File,
    io::BufReader,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::{
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterResponse, GetDocResponse, GetResourceResponse, OpExecResponse,
        PlanResponseElement, SkeletonResponse, TaskExecResponse, VirtToPhyResponse,
    },
    diag::DiagnosticResponse,
    util::RON,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum BundleMapFile {
    Bundle,
    ChildOf { parent: PathBuf },
}

impl BundleMapFile {
    pub fn path(prefix: &Path, addr: &Path) -> PathBuf {
        let mut output = prefix.to_path_buf();

        output.push(".bundle");

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
            // If there's no file name at all, we'll just use ".bun.ron"
            // so `new_filename` right now is just "." — that's fine.
            // We'll end up producing something like "./office/east/ec2/us-east-1/.bun.ron"
        }
        new_filename.push(".bun.ron");

        output.push(new_filename);

        output
    }

    pub fn read(prefix: &Path, addr: &Path) -> anyhow::Result<Option<Self>> {
        let bundle_path = Self::path(prefix, addr);

        if bundle_path.is_file() {
            let file = File::open(&bundle_path)?;
            let reader = BufReader::new(file);

            let bundle: Self = RON.from_reader(reader)?;

            return Ok(Some(bundle));
        }

        Ok(None)
    }

    pub fn read_recurse(prefix: &Path, addr: &Path) -> anyhow::Result<Option<Self>> {
        let bundle_path = Self::path(prefix, addr);

        if bundle_path.is_file() {
            let file = File::open(&bundle_path)?;
            let reader = BufReader::new(file);

            let bundle: Self = RON.from_reader(reader)?;

            match &bundle {
                BundleMapFile::ChildOf { parent } => {
                    return Self::read_recurse(prefix, parent);
                }
                BundleMapFile::Bundle => return Ok(Some(bundle)),
            }
        }

        Ok(None)
    }

    pub fn write(&self, prefix: &Path, addr: &Path) -> anyhow::Result<PathBuf> {
        let bundle_path = Self::path(prefix, addr);
        let pretty_config = PrettyConfig::default();

        let contents = RON.to_string_pretty(self, pretty_config)?;

        if let Some(parent) = bundle_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if bundle_path.exists() {
            std::fs::remove_file(&bundle_path)?;
        }

        std::fs::write(&bundle_path, contents)?;

        Ok(bundle_path)
    }

    pub fn write_link(prefix: &Path, child: &Path, parent: &Path) -> anyhow::Result<PathBuf> {
        let child_map = Self::ChildOf {
            parent: parent.to_owned(),
        };
        child_map.write(prefix, child)?;
        Ok(Self::path(prefix, child))
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnbundleResponseElement {
    pub addr: PathBuf,
    pub contents: Vec<u8>,
}

impl UnbundleResponseElement {}

#[async_trait]
pub trait Bundle
where
    Self: Send + Sync,
{
    #[allow(clippy::new_ret_no_self)]
    async fn new(name: &str, prefix: &Path) -> Result<Arc<dyn Bundle>, anyhow::Error>
    where
        Self: Sized;

    async fn init(&self) -> Result<(), anyhow::Error>;

    async fn version(&self) -> Result<String, anyhow::Error>;

    async fn filter(&self, addr: &Path) -> Result<FilterResponse, anyhow::Error>;

    async fn unbundle(&self, addr: &Path, resource: &[u8]) -> anyhow::Result<Vec<UnbundleResponseElement>>;

    async fn get_skeletons(&self) -> Result<Vec<SkeletonResponse>, anyhow::Error> {
        Ok(Vec::new())
    }
    async fn get_docstring(&self, _addr: &Path, _ident: DocIdent) -> anyhow::Result<Option<GetDocResponse>> {
        Ok(None)
    }
    async fn eq(&self, _addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        Ok(a == b)
    }
    async fn diag(&self, _addr: &Path, _a: &[u8]) -> Result<Option<DiagnosticResponse>, anyhow::Error> {
        Ok(None)
    }
}

#[async_trait]
impl Bundle for Arc<dyn Bundle> {
    async fn new(name: &str, prefix: &Path) -> Result<Arc<dyn Bundle>, anyhow::Error> {
        return <Arc<dyn Bundle> as Bundle>::new(name, prefix).await;
    }

    async fn init(&self) -> Result<(), anyhow::Error> {
        Bundle::init(self.as_ref()).await
    }

    async fn version(&self) -> Result<String, anyhow::Error> {
        Bundle::version(self.as_ref()).await
    }

    async fn filter(&self, addr: &Path) -> Result<FilterResponse, anyhow::Error> {
        Bundle::filter(self.as_ref(), addr).await
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> anyhow::Result<Option<GetDocResponse>> {
        Bundle::get_docstring(self.as_ref(), addr, ident).await
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        Bundle::eq(self.as_ref(), addr, a, b).await
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<Option<DiagnosticResponse>, anyhow::Error> {
        Bundle::diag(self.as_ref(), addr, a).await
    }

    async fn unbundle(&self, addr: &Path, resource: &[u8]) -> anyhow::Result<Vec<UnbundleResponseElement>> {
        Bundle::unbundle(self.as_ref(), addr, resource).await
    }
}

#[async_trait]
impl Connector for Arc<dyn Bundle> {
    async fn new(name: &str, prefix: &Path, _outbox: ConnectorOutbox) -> Result<Arc<dyn Connector>, anyhow::Error> {
        let bundle: Arc<dyn Bundle> = <Arc<dyn Bundle + 'static> as Bundle>::new(name, prefix).await?;
        Ok(Arc::new(bundle))
    }

    async fn init(&self) -> Result<(), anyhow::Error> {
        Bundle::init(self).await
    }

    async fn version(&self) -> Result<String, anyhow::Error> {
        Bundle::version(self).await
    }

    async fn filter(&self, addr: &Path) -> Result<FilterResponse, anyhow::Error> {
        Bundle::filter(self, addr).await
    }

    async fn list(&self, _subpath: &Path) -> anyhow::Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }

    async fn subpaths(&self) -> anyhow::Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }

    async fn get(&self, _addr: &Path) -> Result<Option<GetResourceResponse>, anyhow::Error> {
        Ok(None)
    }

    async fn plan(
        &self,
        _addr: &Path,
        _current: Option<Vec<u8>>,
        _desired: Option<Vec<u8>>,
    ) -> Result<Vec<PlanResponseElement>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn op_exec(&self, _addr: &Path, _op: &str) -> Result<OpExecResponse, anyhow::Error> {
        Ok(OpExecResponse {
            outputs: Some(HashMap::new()),
            friendly_message: Some(String::from("Bundle: No-op!")),
        })
    }

    async fn addr_virt_to_phy(&self, addr: &Path) -> Result<VirtToPhyResponse, anyhow::Error> {
        Ok(VirtToPhyResponse::Present(addr.into()))
    }

    async fn addr_phy_to_virt(&self, addr: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        Ok(Some(addr.into()))
    }

    async fn get_skeletons(&self) -> Result<Vec<SkeletonResponse>, anyhow::Error> {
        Bundle::get_skeletons(self).await
    }

    async fn get_docstring(&self, addr: &Path, ident: DocIdent) -> anyhow::Result<Option<GetDocResponse>> {
        Bundle::get_docstring(self, addr, ident).await
    }

    async fn eq(&self, addr: &Path, a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
        Bundle::eq(self, addr, a, b).await
    }

    async fn diag(&self, addr: &Path, a: &[u8]) -> Result<Option<DiagnosticResponse>, anyhow::Error> {
        Bundle::diag(self, addr, a).await
    }

    async fn task_exec(
        &self,
        _addr: &Path,
        _body: Vec<u8>,
        _arg: Option<Vec<u8>>,
        _state: Option<Vec<u8>>,
    ) -> anyhow::Result<TaskExecResponse> {
        Ok(TaskExecResponse::default())
    }

    async fn unbundle(&self, addr: &Path, bundle: &[u8]) -> anyhow::Result<Vec<UnbundleResponseElement>> {
        Bundle::unbundle(self, addr, bundle).await
    }
}
