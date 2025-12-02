use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use serde::Serialize;

use crate::{
    bundle::UnbundleResponseElement,
    connector::{
        Connector, ConnectorOutbox, DocIdent, FilterResponse, GetDocResponse, GetResourceResponse, OpExecResponse,
        PlanResponseElement, SkeletonResponse, TaskExecResponse, VirtToPhyResponse,
    },
    diag::DiagnosticResponse,
};

#[derive(Debug, Serialize)]
pub enum ConnectorHandleStatus {
    Alive { memory: u64, cpu_usage: f32 },
    Dead,
}

#[async_trait]
pub trait ConnectorHandle: Connector {
    async fn status(&self) -> ConnectorHandleStatus;

    async fn kill(&self) -> anyhow::Result<()>;
}

#[async_trait]
impl ConnectorHandle for Arc<dyn ConnectorHandle> {
    async fn status(&self) -> ConnectorHandleStatus {
        ConnectorHandle::status(self.as_ref()).await
    }

    async fn kill(&self) -> anyhow::Result<()> {
        ConnectorHandle::kill(self.as_ref()).await
    }
}

#[async_trait]
impl Connector for Arc<dyn ConnectorHandle> {
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

    async fn task_exec(
        &self,
        addr: &Path,
        body: Vec<u8>,
        arg: Option<Vec<u8>>,
        state: Option<Vec<u8>>,
    ) -> anyhow::Result<TaskExecResponse> {
        Connector::task_exec(self.as_ref(), addr, body, arg, state).await
    }

    async fn unbundle(&self, addr: &Path, bundle: &[u8]) -> anyhow::Result<Vec<UnbundleResponseElement>> {
        Connector::unbundle(self.as_ref(), addr, bundle).await
    }
}
