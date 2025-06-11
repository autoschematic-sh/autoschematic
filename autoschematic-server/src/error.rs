use actix_web::{body::BodyLimitExceeded, error::PayloadError, HttpResponse, ResponseError};
use anyhow::anyhow;
use autoschematic_core::error::AutoschematicError;
use std::fmt::{self, Debug};

#[derive(Debug)]
pub enum AutoschematicServerErrorType {
    /// Error when parsing an invalid connector string
    InvalidConnectorString(String),

    /// Error when parsing an invalid keystore string
    InvalidKeystoreString(String),
    
    /// Error when parsing an invalid lock string
    InvalidLockString(String),

    /// Internal service error wrapping anyhow::Error
    InternalError(anyhow::Error),
    
    /// Error when service is not installed on repository
    NotInstalled,

    /// Error when service is not installed on repository
    NotMergeable,
    
    /// Error when a required HTTP header is missing
    MissingHeader(String),

    /// Error when an HTTP header value is invalid
    InvalidHeaderValue(String),

    RepoLocked(String),
    
    /// Error when required configuration is missing or invalid
    ConfigurationError {
        /// Name of the configuration item
        name: String,
        /// Description of the error
        message: String,
    },
}

/// Main error type for the Autoschematic service
#[derive(Debug)]
pub struct AutoschematicServerError {
    pub kind: AutoschematicServerErrorType,
}

impl fmt::Display for AutoschematicServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            AutoschematicServerErrorType::InvalidConnectorString(name) => {
                write!(f, "Invalid Connector String: {}", name)
            }
            AutoschematicServerErrorType::InvalidKeystoreString(name) => {
                write!(f, "Invalid Keystore String: {}", name)
            }
            AutoschematicServerErrorType::InvalidLockString(name) => {
                write!(f, "Invalid Lock String: {}", name)
            }
            AutoschematicServerErrorType::NotInstalled => write!(f, "Not installed on repo"),
            AutoschematicServerErrorType::NotMergeable => write!(f, "Pull request is not mergeable"),
            AutoschematicServerErrorType::InternalError(e) => write!(f, "Internal Error: {:#}", e),
            AutoschematicServerErrorType::MissingHeader(header_name) => {
                write!(f, "Missing Header: {}", header_name)
            }
            AutoschematicServerErrorType::InvalidHeaderValue(header_name) => {
                write!(f, "Invalid Header Value: {}", header_name)
            }
            AutoschematicServerErrorType::ConfigurationError { name, message } => {
                write!(f, "Configuration Error - {}: {}", name, message)
            }
            AutoschematicServerErrorType::RepoLocked(name) => {
                write!(f, "Repository Locked: {}", name)
            }
        }
    }
}

impl std::error::Error for AutoschematicServerError {}

impl ResponseError for AutoschematicServerError {
    fn error_response(&self) -> HttpResponse {
        match &self.kind {
            AutoschematicServerErrorType::InvalidConnectorString(message) => {
                tracing::warn!("Invalid Connector String: {}", message);
                HttpResponse::NotFound().body(format!("Invalid Connector String: {}", message))
            }
            AutoschematicServerErrorType::NotInstalled => {
                tracing::warn!("Not installed on repo");
                HttpResponse::InternalServerError().body("Not installed on repo")
            }
            AutoschematicServerErrorType::InternalError(e) => {
                tracing::error!("Application error: {:#?}", e);
                tracing::error!("Application error: {}", e);
                tracing::error!("Application error: {:#}", e);
                tracing::error!("Application error: {:?}", e);
                HttpResponse::InternalServerError().body("Something went wrong")
            }
            // AutoschematicErrorType::MissingHeader(header_name) => {
            //     tracing::warn!("Missing Header: {}", header_name);
            //     HttpResponse::BadRequest().body(format!("Missing Header: {}", header_name))
            // }
            // AutoschematicErrorType::InvalidHeaderValue(header_name) => {
            //     tracing::warn!("Invalid Header Value: {}", header_name);
            //     HttpResponse::BadRequest().body(format!("Invalid Header Value: {}", header_name))
            // }
            // AutoschematicErrorType::ConfigurationError { name, message } => {
            //     tracing::error!("Configuration error - {}: {}", name, message);
            //     HttpResponse::InternalServerError().body(format!("Configuration error: {}", message))
            // }
            e => {
                tracing::error!("{:#?}", e);
                HttpResponse::InternalServerError().body("Internal Error".to_string())
            }
        }
    }
}

// Error conversion implementations

impl From<anyhow::Error> for AutoschematicServerError {
    fn from(err: anyhow::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err),
        }
    }
}

impl From<AutoschematicError> for AutoschematicServerError {
    fn from(err: AutoschematicError) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<askama::Error> for AutoschematicServerError {
    fn from(err: askama::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<octocrab::Error> for AutoschematicServerError {
    fn from(err: octocrab::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<std::io::Error> for AutoschematicServerError {
    fn from(err: std::io::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<git2::Error> for AutoschematicServerError {
    fn from(err: git2::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<serde_json::Error> for AutoschematicServerError {
    fn from(err: serde_json::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<PayloadError> for AutoschematicServerError {
    fn from(err: PayloadError) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<hyper::header::ToStrError> for AutoschematicServerError {
    fn from(err: hyper::header::ToStrError) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<regex::Error> for AutoschematicServerError {
    fn from(err: regex::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<tera::Error> for AutoschematicServerError {
    fn from(err: tera::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<reqwest::Error> for AutoschematicServerError {
    fn from(err: reqwest::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<actix_web::http::header::ToStrError> for AutoschematicServerError {
    fn from(err: actix_web::http::header::ToStrError) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<actix_web::Error> for AutoschematicServerError {
    fn from(err: actix_web::Error) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(anyhow!("{}", err)),
        }
    }
}

impl From<BodyLimitExceeded> for AutoschematicServerError {
    fn from(err: BodyLimitExceeded) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}

impl From<tokio::sync::TryLockError> for AutoschematicServerError {
    fn from(err: tokio::sync::TryLockError) -> Self {
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(err.into()),
        }
    }
}