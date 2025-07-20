use std::{collections::HashMap, path::PathBuf};

use anyhow::bail;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AutoschematicConfig {
    #[serde(default)]
    pub safety_active: Option<bool>,
    pub prefixes: HashMap<String, Prefix>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Prefix {
    pub connectors: Vec<Connector>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub resource_group: Option<String>,
    #[serde(default)]
    pub tasks: Vec<Task>,
    // TODO merge this with the rest of the connector env!
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub read_secrets: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
pub enum Protocol {
    #[default]
    Tarpc,
    Grpc,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
/// Represents the precise type and installation of a given Connector instance.
pub enum Spec {
    Binary {
        path: PathBuf,
        #[serde(default)]
        protocol: Protocol,
    },
    Cargo {
        name: String,
        #[serde(default)]
        version: Option<String>,
        #[serde(default)]
        binary: Option<String>,
        #[serde(default)]
        git: Option<String>,
        #[serde(default)]
        features: Option<Vec<String>>,
        #[serde(default)]
        protocol: Protocol,
    },
    CargoLocal {
        path: PathBuf,
        #[serde(default)]
        binary: Option<String>,
        #[serde(default)]
        features: Option<Vec<String>>,
        #[serde(default)]
        protocol: Protocol,
    },
    TypescriptLocal {
        path: PathBuf,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct SpecCommand {
    pub binary: PathBuf,
    pub args: Vec<String>,
}

impl Spec {
    pub fn protocol(&self) -> Protocol {
        match self {
            Spec::Binary { path, protocol } => protocol.clone(),
            Spec::Cargo {
                name,
                version,
                binary,
                git,
                features,
                protocol,
            } => protocol.clone(),
            Spec::CargoLocal {
                path,
                binary,
                features,
                protocol,
            } => protocol.clone(),
            Spec::TypescriptLocal { path } => Protocol::Grpc,
        }
    }
    pub fn pre_command(&self) -> anyhow::Result<Option<SpecCommand>> {
        match self {
            Spec::CargoLocal {
                path, binary, features, ..
            } => {
                let manifest_path = path.join("Cargo.toml");
                if !manifest_path.is_file() {
                    bail!("Spec::pre_command: No Cargo.toml under {}", path.display())
                }

                let mut args: Vec<String> = vec!["build", "--release", "--manifest-path", manifest_path.to_str().unwrap()]
                    .into_iter()
                    .map(|s| String::from(s))
                    .collect();

                if let Some(binary) = binary {
                    args.append(&mut vec![String::from("--bin"), binary.to_string()]);
                }
                if let Some(features) = features.to_owned()
                    && !features.is_empty()
                {
                    args.append(&mut vec![String::from("--features"), features.join(",").to_string()]);
                }
                Ok(Some(SpecCommand {
                    binary: "cargo".into(),
                    args,
                }))
            }
            _ => Ok(None),
        }
    }

    pub fn command(&self) -> anyhow::Result<SpecCommand> {
        match self {
            Spec::Binary { path, protocol } => {
                let mut binary_path = path.clone();
                if !binary_path.is_file() {
                    binary_path = which::which(binary_path)?;
                }

                if !binary_path.is_file() {
                    bail!("launch_server_binary: {}: not found", binary_path.display())
                }
                // let mut command = tokio::process::Command::new(binary_path);
                // let args = [shortname.into(), prefix.into(), socket.clone(), error_dump.clone()];
                // command.args(args);
                // command.stdout(io::stderr());
                // command
                Ok(SpecCommand {
                    binary: binary_path,
                    args: Vec::new(),
                })
            }
            Spec::Cargo { name, .. } => {
                let cargo_home = match std::env::var("CARGO_HOME") {
                    Ok(p) => PathBuf::from(p),
                    Err(_) => {
                        let Ok(home) = std::env::var("HOME") else {
                            bail!("$HOME not set!");
                        };
                        PathBuf::from(home).join(".cargo")
                    }
                };

                // TODO Also parse `binary` and check .cargo/.cargo.toml
                let binary_path = cargo_home.join("bin").join(name);

                if !binary_path.is_file() {
                    bail!("launch_server_binary: {}: not found", binary_path.display())
                }
                // let mut command = tokio::process::Command::new(binary_path);
                // let args = [shortname.into(), prefix.into(), socket.clone(), error_dump.clone()];
                // command.args(args);
                // command.stdout(io::stderr());
                // command
                Ok(SpecCommand {
                    binary: binary_path,
                    args: Vec::new(),
                })
            }
            Spec::CargoLocal {
                path, binary, features, ..
            } => {
                let manifest_path = path.join("Cargo.toml");
                if !manifest_path.is_file() {
                    bail!("Spec::pre_command: No Cargo.toml under {}", path.display())
                }

                let mut args: Vec<String> = vec!["run", "--release", "--manifest-path", manifest_path.to_str().unwrap()]
                    .into_iter()
                    .map(|s| String::from(s))
                    .collect();

                if let Some(binary) = binary {
                    args.append(&mut vec![String::from("--bin"), binary.to_string()]);
                }
                if let Some(features) = features.to_owned()
                    && !features.is_empty()
                {
                    args.append(&mut vec![String::from("--features"), features.join(",").to_string()]);
                }
                Ok(SpecCommand {
                    binary: "cargo".into(),
                    args,
                })
            }
            Spec::TypescriptLocal { path } => {
                if !path.is_file() {
                    bail!("launch_server_binary: {}: not found", path.display())
                }
                let args = vec![
                    path.to_string_lossy().to_string(),
                    // shortname.into(),
                    // prefix.into(),
                    // socket.clone(),
                    // error_dump.clone(),
                ];
                // command.args(args);
                // command.stdout(io::stderr());
                // command
                Ok(SpecCommand {
                    binary: "tsx".into(),
                    args,
                })
            }
        }
    }
}

// TODO we'll also define ConnectorSet, a standalone file with a set of Connectors,
// to allow prefixes to share common sets of connectors.

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Connector {
    pub shortname: String,
    pub spec: Spec,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub read_secrets: Vec<String>,
}

// #[derive(Debug, Default, Deserialize, Serialize)]
// #[serde(deny_unknown_fields)]
/// Represents the on-disk format of autoschematic.ron .
/// Variants of this may be created if the on-disk format ever needs to be modified,
/// and tools can try each variant in sequence in order to maintain backwards-compatibility.
// pub struct AutoschematicConfigFile {
//     pub prefixes: HashMap<String, PrefixDef>,
// }

// impl From<AutoschematicConfigFile> for AutoschematicConfig {
//     fn from(value: AutoschematicConfigFile) -> Self {
//         let autoschematic_config = AutoschematicConfig {
//             prefixes: value.prefixes,
//         };

//         autoschematic_config
//     }
// }

impl AutoschematicConfig {
    pub fn resource_group_map(&self) -> HashMap<String, Vec<PathBuf>> {
        let mut res = HashMap::new();
        for (prefix_name, prefix) in &self.prefixes {
            if let Some(resource_group) = &prefix.resource_group {
                if !res.contains_key(resource_group) {
                    res.insert(resource_group.to_string(), Vec::new());
                }

                if let Some(prefixes) = res.get_mut(resource_group) {
                    prefixes.push(PathBuf::from(prefix_name));
                }
            }
        }

        tracing::debug!("Resource group map: {:?}", res);

        res
    }
}
