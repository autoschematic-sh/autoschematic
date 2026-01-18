use std::{collections::HashMap, path::PathBuf};

use anyhow::bail;
use autoschematic_macros::FieldTypes;
use documented::{Documented, DocumentedFields};
use serde::{Deserialize, Serialize};

use crate::macros::FieldTypes;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Documented, DocumentedFields, FieldTypes)]
#[serde(deny_unknown_fields)]
/// The root Autoschematic config. This should be in a file called "autoschematic.ron" at the root of a git repo.
pub struct AutoschematicConfig {
    /// Autoschematic can divide repos up into prefixes to allow multi-team and multi-account workflows.
    /// A prefix just represents a folder, or nested folders. If you don't wish to divide your repo
    /// up at all, you can just use the prefix "/".
    /// For example:
    /// ```ignore
    /// prefixes: {
    ///     "/": Prefix(...),
    /// }
    /// ```
    /// ```ignore
    /// prefixes: {
    ///     "team/backend": Prefix(...),
    ///     "team/frontend": Prefix(...),
    /// }
    /// ```
    /// ```ignore
    /// prefixes: {
    ///     "office/taipei": Prefix(...),
    ///     "office/london": Prefix(...),
    ///     "office/sanfrancisco": Prefix(...),
    /// }
    /// ```
    pub prefixes: HashMap<String, Prefix>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Documented, DocumentedFields, FieldTypes)]
#[serde(deny_unknown_fields)]
/// A Prefix object defines the set of connectors that are installed in that prefix, as well as task definitions, common environment variables,
/// and metadata.
pub struct Prefix {
    /// A list of Connector(...) definitions. See the [Connector Catalogue](https://autoschematic.sh/catalogue/) for
    /// the set of connectors that you can install and use, along with the syntax to include them here.
    pub connectors: Vec<Connector>,
    #[serde(default)]
    /// [Optional] A human-readable description of the prefix.
    pub description: Option<String>,
    #[serde(default)]
    /// When two prefixes A and B have the same resource_group string, `autoschematic import` will
    /// not import resources in prefix A that already exist in prefix B. This is useful for
    /// having two prefixes that share the same AWS account, for example.
    pub resource_group: Option<String>,

    #[serde(default)]
    /// [Optional] A list of Task(...) definitions. Note: the task API is currently under review and is less stable than the rest of the core API.
    pub tasks: Vec<AuxTask>,
    /// [Optional] An env file path (like ".env") to read environment variables from.
    #[serde(default)]
    pub env_file: Option<String>,
    /// [Optional] A map of common environment variables shared between all connectors in this prefix. Takes precedence over env_file.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Documented, DocumentedFields, FieldTypes)]
#[serde(deny_unknown_fields)]
/// An auxilary task definition.
pub struct AuxTask {
    /// The identifier of the aux task to enable.
    pub name: String,
    /// A free-form, human-readable description of this task.
    #[serde(default)]
    pub description: Option<String>,
    /// [Optional] A map of environment variables for this connector. Takes precedence over env_file and Prefix.env on a per-variable basis.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// [Optional] An env file path (like ".env") to read environment variables from.
    /// Takes precedence over Prefix.env and Prefix.env_file on a per-variable basis.
    #[serde(default)]
    pub env_file: Option<String>,
    // TODO where do we plug this in now?
    // #[serde(default)]
    // pub read_secrets: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Documented)]
/// Defines what protocol the connector will run under. Connectors will break if set
/// to the wrong protocol; you shouldn't normally need to set this.
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
        cargo: Option<String>,
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
            Spec::Binary { protocol, .. } => protocol.clone(),
            Spec::Cargo { protocol, .. } => protocol.clone(),
            Spec::CargoLocal { protocol, .. } => protocol.clone(),
            Spec::TypescriptLocal { .. } => Protocol::Grpc,
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
                    .map(String::from)
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
            Spec::Binary { path, .. } => {
                let binary_path = path.clone();
                // if !binary_path.is_file() {
                //     binary_path = which::which(binary_path)?;
                // }

                // if !binary_path.is_file() {
                //     bail!("launch_server_binary: {}: not found", binary_path.display())
                // }
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
                path,
                binary,
                features,
                cargo,
                ..
            } => {
                let manifest_path = path.join("Cargo.toml");
                if !manifest_path.is_file() {
                    bail!("Spec::pre_command: No Cargo.toml under {}", path.display())
                }

                let mut args: Vec<String> = vec!["run", "--release", "--manifest-path", manifest_path.to_str().unwrap()]
                    .into_iter()
                    .map(String::from)
                    .collect();

                if let Some(binary) = binary {
                    args.append(&mut vec![String::from("--bin"), binary.to_string()]);
                }
                if let Some(features) = features.to_owned()
                    && !features.is_empty()
                {
                    args.append(&mut vec![String::from("--features"), features.join(",").to_string()]);
                }

                let binary = if let Some(cargo) = cargo {
                    cargo.into()
                } else {
                    "cargo".into()
                };

                Ok(SpecCommand {
                    binary,
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Documented, DocumentedFields, FieldTypes)]
#[serde(deny_unknown_fields)]
/// Connectors are responsible for managing different kinds of resources as code. Connectors run as a lightweight
/// server process over tarpc or grpc.
pub struct Connector {
    /// The shortname is used to refer to the connector in the command line
    pub shortname: String,
    /// The spec should be taken from the [Connector Catalogue](https://autoschematic.sh/catalogue/).
    pub spec: Spec,
    #[serde(default)]
    /// A map of environment variables to set. Note that
    /// environment variables from the host are not passed through unless explicitly
    /// set here like so:
    /// ```ignore
    /// env: {
    ///    ...
    ///   "GITHUB_TOKEN": "env://GITHUB_TOKEN",
    ///   ...
    /// }
    /// ```
    pub env: HashMap<String, String>,
    /// [Optional] An env file path (like ".env") to read environment variables from.
    #[serde(default)]
    pub env_file: Option<String>,
    // #[serde(default)]
    // The set of secrets that this connector is allowed to unseal at runtime.
    // TODO where do we plug this in now?
    // pub read_secrets: Vec<String>,
}

// #[derive(Debug, Default, Deserialize, Serialize)]
// #[serde(deny_unknown_fields)]
// Represents the on-disk format of autoschematic.ron .
// Variants of this may be created if the on-disk format ever needs to be modified,
// and tools can try each variant in sequence in order to maintain backwards-compatibility.
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
