use std::path::Path;

use crate::{
    connector::{ConnectorOp, ResourceAddress},
    error::{AutoschematicError, AutoschematicErrorType},
};

pub fn invalid_addr_path(path: &Path) -> anyhow::Error {
    AutoschematicError {
        kind: AutoschematicErrorType::InvalidAddr(path.to_path_buf()),
    }
    .into()
}

pub fn invalid_addr(addr: &impl ResourceAddress) -> anyhow::Error {
    AutoschematicError {
        kind: AutoschematicErrorType::InvalidAddr(addr.to_path_buf()),
    }
    .into()
}

pub fn invalid_op(addr: &impl ResourceAddress, op: &impl ConnectorOp) -> anyhow::Error {
    AutoschematicError {
        kind: AutoschematicErrorType::InvalidOp(addr.to_path_buf(), format!("{op:#?}")),
    }
    .into()
}
