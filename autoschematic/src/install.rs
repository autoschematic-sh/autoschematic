use std::{collections::HashMap, path::PathBuf, process::Stdio};

use anyhow::bail;
use autoschematic_core::config::AutoschematicConfig;
use toml::Table;

use crate::config::load_autoschematic_config;

pub async fn install() -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    cargo_install_missing(&config).await?;

    Ok(())
}

pub async fn cargo_install_missing(config: &AutoschematicConfig) -> anyhow::Result<()> {
    let cargo_home = match std::env::var("CARGO_HOME") {
        Ok(p) => PathBuf::from(p),
        Err(_) => {
            let Ok(home) = std::env::var("HOME") else {
                bail!("$HOME not set!");
            };
            PathBuf::from(home).join(".cargo")
        }
    };

    let cargo_registry = std::fs::read_to_string(cargo_home.join(".crates.toml"))?.parse::<Table>()?;
    let Some(cargo_registry_v1) = cargo_registry.get("v1") else {
        bail!("No key `v1` in $CARGO_HOME/.crates.toml");
    };

    let pkg_table = cargo_registry_v1.as_table().unwrap();

    type PackageName = String;
    type Version = String;
    type Binary = String;
    let mut pkg_map: HashMap<PackageName, HashMap<Version, Vec<Binary>>> = HashMap::new();

    for (pkg, binaries) in pkg_table {
        let pkg_name: Vec<&str> = pkg.split(" ").collect();
        if !(pkg_map.contains_key(pkg_name[0])) {
            pkg_map.insert(pkg_name[0].to_string(), HashMap::new());
        }
        let binaries: Vec<String> = binaries
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();

        pkg_map
            .get_mut(pkg_name[0])
            .unwrap()
            .insert(pkg_name[1].to_string(), binaries);
    }
    
    for (prefix_name, prefix) in &config.prefixes {
        for connector in &prefix.connectors {
            match &connector.spec {
                autoschematic_core::config::Spec::Cargo {
                    name,
                    version,
                    binary,
                    git,
                    features,
                    protocol,
                } => {
                    
                    
                    // TODO check pkg_map and skip existing with same version, features
                    println!("Installing {name}");

                    let mut command = tokio::process::Command::new("cargo");

                    command.args(["install"]);

                    if let Some(git) = git {
                        command.args(["--git", git]);
                    } else {
                        if let Some(version) = version {
                            command.args([format!("{name}@{version}")]);
                        } else {
                            command.args([name]);
                        }
                    }

                    if let Some(features) = features
                        && !features.is_empty()
                    {
                        command.args(["--features", &features.join(",")]);
                    }

                    let output = command.stdin(Stdio::inherit()).stdout(Stdio::inherit()).output().await?;

                    if !output.status.success() {
                        bail!("Pre-command failed: {:?}: {}", command, output.status)
                    }
                }
                _ => continue,
            }
        }
    }
    Ok(())
}
