use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::{
    bundle::{BundleMapFile, UnbundleResponseElement},
    config::Spec,
    connector::{OpExecResponse, PlanResponseElement},
    error::ErrorMessage,
    template::ReadOutput,
};
use anyhow::bail;
use serde::{Deserialize, Serialize};
//
// A PlanReport outlines, for a given plan run at connector:prefix:addr:
// The error, if any, or
// The list of ConnectorOps along with their human-readable descriptions
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct PlanReport {
    pub prefix: PathBuf,
    pub connector_shortname: String,
    pub connector_spec: Option<Spec>,
    pub connector_env: HashMap<String, String>,
    pub virt_addr: PathBuf,
    /// Optional: if different to virt_addr, represents the \
    /// result of Connector::addr_virt_to_phy()
    pub phy_addr: Option<PathBuf>,
    pub connector_ops: Vec<PlanResponseElement>,
    pub reads_outputs: Vec<ReadOutput>,
    // TODO we don't distinguish between missing outputs used to template and missing parent resource
    // outputs to compute Connector::addr_virt_to_phy(). Should we?
    pub missing_outputs: Vec<ReadOutput>,
    pub error: Option<ErrorMessage>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PlanReportSet {
    pub overall_success: bool,
    pub apply_success: bool,
    pub plan_reports: Vec<PlanReport>,
    pub deferred_count: usize,
    pub object_count: usize,
    pub deferred_pending_outputs: HashSet<ReadOutput>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct ApplyReport {
    pub connector_shortname: String,
    pub prefix: PathBuf,
    pub virt_addr: PathBuf,
    pub phy_addr: Option<PathBuf>,
    pub outputs: Vec<OpExecResponse>,
    pub wrote_files: Vec<PathBuf>,
    pub error: Option<ErrorMessage>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ApplyReportSet {
    pub connector_shortname: String,
    pub prefix: PathBuf,
    pub virt_addr: PathBuf,
    pub phy_addr: Option<PathBuf>,
    pub overall_success: bool,
    pub apply_reports: Vec<ApplyReport>,
    pub error: Option<ErrorMessage>,
}

// A PlanReport outlines, for a given plan run at connector:prefix:addr:
// The error, if any, or
// The list of ConnectorOps along with their human-readable descriptions
#[derive(Serialize, Deserialize, Clone)]
pub struct PlanReportOld {
    pub connector_name: String,
    pub connector_spec: Spec,
    pub connector_env: HashMap<String, String>,
    pub prefix: String,
    pub virt_addr: PathBuf,
    pub phy_addr: Option<PathBuf>,
    pub connector_ops: Vec<PlanResponseElement>,
    pub reads_outputs: Vec<ReadOutput>,
    pub error: Option<ErrorMessage>,
}

// An ApplyReport outlines, for a given apply run at connector:prefix:addr:
// The error, if any, or
// The list of ConnectorOps along with their human-readable descriptions
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct ApplyReportOld {
    pub connector_shortname: String,
    pub prefix: PathBuf,
    pub virt_addr: PathBuf,
    pub phy_addr: Option<PathBuf>,
    pub outputs: Vec<OpExecResponse>,
    pub wrote_files: Vec<PathBuf>,
    pub error: Option<ErrorMessage>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct UnbundleReport {
    pub prefix: PathBuf,
    pub addr: PathBuf,

    pub missing_outputs: Vec<ReadOutput>,
    pub elements: Option<Vec<UnbundleResponseElement>>,
}

impl UnbundleReport {
    pub async fn write_to_disk(&self, overbundle: bool, git_stage: bool) -> anyhow::Result<()> {
        let Some(ref elements) = self.elements else { return Ok(()) };

        for element in elements {
            let output_path = self.prefix.join(element.addr.clone());

            if !overbundle && output_path.is_file() {
                if let Some(bundle_map) = BundleMapFile::read(&self.prefix, &element.addr)? {
                    match bundle_map {
                        BundleMapFile::Bundle => {}
                        BundleMapFile::ChildOf { parent } => {
                            if parent != self.addr {
                                bail!(
                                    "UnbundleReport::write_to_disk(): {} exists but belongs to a different bundle, and overbundle is not set.",
                                    output_path.display()
                                )
                            }
                        }
                    }
                } else {
                    bail!(
                        "UnbundleReport::write_to_disk(): {} exists but is not in a bundle, and overbundle is not set.",
                        output_path.display()
                    )
                }
            }

            tokio::fs::write(output_path, &element.contents).await?;
            BundleMapFile::write_link(&self.prefix, &element.addr, &self.addr)?;
        }

        Ok(())
    }
}
