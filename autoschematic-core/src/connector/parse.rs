use std::path::PathBuf;

use regex::Regex;

use crate::error::{AutoschematicError, AutoschematicErrorType};

use super::r#type::ConnectorType;

pub fn parse_connector_name(name: &str) -> Result<ConnectorType, anyhow::Error> {
    // Match a Connector name.
    // Connector names take the form:
    // {type}:{path}
    // Where connector implementations may further interpret `path`
    // to enable sub-connector implementations.
    // E.G. python:modules/snowflake.py:SnowflakeConnector
    let re = Regex::new(r"^(?<type>[^:]+):(?<path>.+)$")?;

    let Some(caps) = re.captures(name) else {
        return Err(AutoschematicError {
            kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
        }
        .into());
    };

    match &caps["type"] {
        // #[cfg(feature = "python")]
        "python" => {
            let path = &caps["path"];

            let re = Regex::new(r"^(?<python_path>[^:]+):(?<classname>.+)$")?;
            let Some(caps) = re.captures(path) else {
                return Err(AutoschematicError {
                    kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
                }
                .into());
            };

            let python_path = &caps["python_path"];
            let classname = &caps["classname"];
            Ok(ConnectorType::Python(python_path.into(), classname.into()))
        }
        "binary-tarpc" => {
            // Run a Connector as sandboxed executable binary over Tarpc.
            // Format: "binary:path/of/binary:ConnectorName"
            let path = &caps["path"];

            let re = Regex::new(r"^(?<binary_path>[^:]+):(?<shortname>.+)$")?;
            let Some(caps) = re.captures(path) else {
                return Err(AutoschematicError {
                    kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
                }
                .into());
            };

            let binary_path = &caps["binary_path"];
            let shortname = &caps["shortname"];
            Ok(ConnectorType::BinaryTarpc(
                PathBuf::from(binary_path),
                shortname.into(),
            ))
        }
        "lock" => {
            // Format: "lock:EntryName:ConnectorName"
            let path = &caps["path"];

            let re = Regex::new(r"^(?<entry_name>[^:]+):(?<shortname>.+)$")?;
            let Some(caps) = re.captures(path) else {
                return Err(AutoschematicError {
                    kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
                }
                .into());
            };

            let entry_name = &caps["entry_name"];
            let shortname = &caps["shortname"];
            Ok(ConnectorType::LockFile(
                PathBuf::from(entry_name),
                shortname.into(),
            ))
        }
        _ => Err(AutoschematicError {
            kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
        }
        .into()),
    }
}

pub fn connector_shortname(name: &str) -> Result<String, AutoschematicError> {
    match name {
        other => {
            // Match a Connector name.
            // Connector names take the form:
            // {type}:{path}
            // Where connector implementations may further interpret `path`
            // to enable sub-connector implementations.
            // E.G. python:modules/snowflake.py:SnowflakeConnector
            let re = Regex::new(r"^(?<type>[^:]+):(?<path>.+)$")?;

            let Some(caps) = re.captures(other) else {
                return Err(AutoschematicError {
                    kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
                });
            };

            match &caps["type"] {
                #[cfg(feature = "python")]
                "python" => Ok("python-custom".into()),
                // "dylib" => Ok((
                //     DylibConnector::new(&caps["path"], prefix, outbox).await?,
                //     inbox,
                // )),
                "binary-tarpc" => {
                    // Run a Connector as sandboxed executable binary over Tarpc.
                    // Format: "binary:path/of/binary:ConnectorName"
                    let path = &caps["path"];

                    let re = Regex::new(r"^(?<binary_path>[^:]+):(?<name>.+)$")?;
                    let Some(caps) = re.captures(path) else {
                        return Err(AutoschematicError {
                            kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
                        }
                        .into());
                    };

                    // let binary_path = &caps["binary_path"];
                    let name = &caps["name"];

                    Ok(String::from(name))
                }
                _ => Err(AutoschematicError {
                    kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
                }),
            }
        }
    }
}
