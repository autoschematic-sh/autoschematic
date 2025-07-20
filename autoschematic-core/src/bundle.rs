use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterResponse, GetDocResponse, GetResourceResponse, OpExecResponse,
        PlanResponseElement, SkeletonResponse, VirtToPhyResponse,
    },
    diag::DiagnosticResponse,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnbundleResponseElement {
    pub filename: PathBuf,
    pub file_contents: String,
}

#[async_trait]
pub trait Bundle: Connector {
    async fn new(name: &str, prefix: &Path) -> Result<Arc<dyn Bundle>, anyhow::Error>
    where
        Self: Sized;

    async fn init(&self) -> Result<(), anyhow::Error>;

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
        let bundle: Arc<dyn Bundle> = <Arc<(dyn Bundle + 'static)> as Bundle>::new(name, prefix).await?;
        Ok(bundle)
    }

    async fn init(&self) -> Result<(), anyhow::Error> {
        Bundle::init(self).await
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

    async fn unbundle(&self, addr: &Path, bundle: &[u8]) -> anyhow::Result<Vec<UnbundleResponseElement>> {
        Bundle::unbundle(self, addr, bundle).await
    }
}
