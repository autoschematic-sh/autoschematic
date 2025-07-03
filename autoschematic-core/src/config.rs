use std::{collections::HashMap, path::PathBuf};

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
}

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
        res
    }
}
