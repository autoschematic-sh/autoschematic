use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::{
    config::Spec,
    connector::{OpExecOutput, OpPlanOutput},
    error::ErrorMessage,
    template::ReadOutput,
};
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
    pub connector_ops: Vec<OpPlanOutput>,
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
    pub outputs: Vec<OpExecOutput>,
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
    pub connector_ops: Vec<OpPlanOutput>,
    pub reads_outputs: Vec<ReadOutput>,
    pub error: Option<ErrorMessage>,
}

pub struct PlanReportSetOld {
    pub overall_success: bool,
    pub apply_success: bool,
    pub plan_reports: Vec<PlanReportOld>,
    pub object_count: usize,
    pub deferred_count: usize,
    pub deferred_pending_outputs: HashSet<ReadOutput>,
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
    pub outputs: Vec<OpExecOutput>,
    pub wrote_files: Vec<PathBuf>,
    pub error: Option<ErrorMessage>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ApplyReportSetOld {
    pub overall_success: bool,
    pub apply_reports: Vec<ApplyReportOld>,
}
