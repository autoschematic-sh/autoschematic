#[cfg(feature = "python")]
use pyo3::FromPyObject;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[repr(u8)]
pub enum DiagnosticSeverity {
    ERROR = 1u8,
    WARNING = 2u8,
    INFORMATION = 3u8,
    HINT = 4u8,
}

/// 1-indexed position of the start or end of a diagnostic,
/// essentially a given cursor point within a file.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "python", derive(FromPyObject))]
pub struct DiagnosticPosition {
    pub line: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "python", derive(FromPyObject))]
pub struct DiagnosticSpan {
    pub start: DiagnosticPosition,
    pub end: DiagnosticPosition,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "python", derive(FromPyObject))]
pub struct Diagnostic {
    pub severity: u8,
    pub span: DiagnosticSpan,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[cfg_attr(feature = "python", derive(FromPyObject))]
pub struct DiagnosticResponse {
    pub diagnostics: Vec<Diagnostic>,
}
