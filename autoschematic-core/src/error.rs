use std::fmt::{self, Debug};

use std::path::PathBuf;
use std::{error::Error, fmt::Display, sync::PoisonError};

use serde::{Deserialize, Serialize};

use crate::connector::Connector;

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct ErrorMessage {
    pub msg: String,
}

impl From<anyhow::Error> for ErrorMessage {
    fn from(value: anyhow::Error) -> Self {
        ErrorMessage {
            msg: format!("{:#}", value),
        }
    }
}

impl<C> From<PoisonError<std::sync::RwLockReadGuard<'_, C>>> for ErrorMessage {
    fn from(value: PoisonError<std::sync::RwLockReadGuard<'_, C>>) -> Self {
        ErrorMessage { msg: value.to_string() }
    }
}

impl From<PoisonError<std::sync::MutexGuard<'_, Box<(dyn Connector + 'static)>>>> for ErrorMessage {
    fn from(value: PoisonError<std::sync::MutexGuard<'_, Box<(dyn Connector + 'static)>>>) -> Self {
        ErrorMessage { msg: value.to_string() }
    }
}

impl Error for ErrorMessage {}

impl Display for ErrorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.msg, f)
    }
}

#[derive(Debug)]
pub enum AutoschematicErrorType {
    /// Error when parsing an invalid connector string
    InvalidConnectorString(String),

    /// Error when parsing an invalid keystore string
    InvalidKeystoreString(String),

    /// Error when parsing an invalid lock string
    InvalidLockString(String),

    InvalidAddr(PathBuf),

    InvalidOp(PathBuf, String),

    /// Internal service error wrapping anyhow::Error
    InternalError(anyhow::Error),
}

#[derive(Debug)]
pub struct AutoschematicError {
    pub kind: AutoschematicErrorType,
}

impl fmt::Display for AutoschematicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            AutoschematicErrorType::InvalidConnectorString(name) => {
                write!(f, "Invalid Connector String: {}", name)
            }
            AutoschematicErrorType::InvalidKeystoreString(name) => {
                write!(f, "Invalid Keystore String: {}", name)
            }
            AutoschematicErrorType::InvalidLockString(name) => {
                write!(f, "Invalid Lock String: {}", name)
            }
            AutoschematicErrorType::InvalidAddr(addr) => {
                write!(f, "Invalid Address: {}", addr.display())
            }
            AutoschematicErrorType::InvalidOp(addr, op) => {
                write!(f, "Invalid ConnectorOp for addr {} : {}", addr.display(), op)
            }
            AutoschematicErrorType::InternalError(e) => write!(f, "Internal Error: {:#}", e),
        }
    }
}

impl std::error::Error for AutoschematicError {}

impl From<anyhow::Error> for AutoschematicError {
    fn from(err: anyhow::Error) -> Self {
        AutoschematicError {
            kind: AutoschematicErrorType::InternalError(err),
        }
    }
}

impl From<regex::Error> for AutoschematicError {
    fn from(err: regex::Error) -> Self {
        AutoschematicError {
            kind: AutoschematicErrorType::InternalError(err.into()),
        }
    }
}

// impl From<octocrab::Error> for AutoschematicError {
//     fn from(err: octocrab::Error) -> Self {
//         AutoschematicError {
//             kind: AutoschematicErrorType::InternalError(err.into()),
//         }
//     }
// }

impl From<std::io::Error> for AutoschematicError {
    fn from(err: std::io::Error) -> Self {
        AutoschematicError {
            kind: AutoschematicErrorType::InternalError(err.into()),
        }
    }
}

impl From<git2::Error> for AutoschematicError {
    fn from(err: git2::Error) -> Self {
        AutoschematicError {
            kind: AutoschematicErrorType::InternalError(err.into()),
        }
    }
}

impl From<serde_json::Error> for AutoschematicError {
    fn from(err: serde_json::Error) -> Self {
        AutoschematicError {
            kind: AutoschematicErrorType::InternalError(err.into()),
        }
    }
}

impl From<hyper::header::ToStrError> for AutoschematicError {
    fn from(err: hyper::header::ToStrError) -> Self {
        AutoschematicError {
            kind: AutoschematicErrorType::InternalError(err.into()),
        }
    }
}

impl From<tokio::sync::TryLockError> for AutoschematicError {
    fn from(err: tokio::sync::TryLockError) -> Self {
        AutoschematicError {
            kind: AutoschematicErrorType::InternalError(err.into()),
        }
    }
}
/*
impl<T: std::error::Error + Send + Sync + 'static> From<T> for AutoschematicError {
    fn from(err: T) -> Self {
        Self {
            kind: AutoschematicErrorType::InternalError(err.into()),
        }
    }
} */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpannedError {
    pub message: String,
    pub severity: String,
    pub position_start: Position,
}
