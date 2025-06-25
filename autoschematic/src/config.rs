use std::path::PathBuf;

use anyhow::{Context, bail};
use autoschematic_core::{
    config::AutoschematicConfig,
    util::{RON, repo_root},
};

pub fn load_autoschematic_config() -> anyhow::Result<AutoschematicConfig> {
    let repo_root = repo_root()?;
    let config_path = repo_root.join("autoschematic.ron");
    let config_body = std::fs::read_to_string(config_path).context("Reading autoschematic.ron")?;
    let config_file: AutoschematicConfig = RON.from_str(&config_body).context("Parsing autoschematic.ron")?;

    for prefix in config_file.prefixes.keys() {
        if prefix.trim() != prefix {
            bail!("Autoschematic prefix \"{}\" contains trailing whitespace.", prefix)
        }
        // The only special case of an absolute prefix path
        if prefix == "/" {
            if config_file.prefixes.len() == 1 {
                continue;
            } else {
                bail!("Autoschematic prefix / is not valid unless it is the only prefix.")
            }
        }
        // ...Otherwise, beat it!
        for component in PathBuf::from(prefix.clone()).components() {
            match component {
                std::path::Component::Prefix(prefix_component) => {
                    bail!(
                        "Autoschematic prefix {} is not valid. Prefixes must not contain Windows path prefixes like {}. (What were you thinking?)",
                        prefix,
                        prefix_component.as_os_str().display()
                    )
                }
                std::path::Component::RootDir => {
                    bail!(
                        "Autoschematic prefix {} is not valid. Prefixes must not be absolute paths.",
                        prefix
                    )
                }
                std::path::Component::CurDir => {
                    bail!(
                        "Autoschematic prefix {} is not valid. Prefixes must not contain relative path components like ./ or ../ .",
                        prefix
                    )
                }
                std::path::Component::ParentDir => {
                    bail!(
                        "Autoschematic prefix {} is not valid. Prefixes must not contain relative path components like ./ or ../ .",
                        prefix
                    )
                }
                std::path::Component::Normal(_) => continue,
            }
        }
    }

    for prefix in config_file.prefixes.keys() {
        for other_prefix in config_file.prefixes.keys() {
            if prefix == other_prefix {
                continue;
            }

            if prefix.starts_with(other_prefix) {
                bail!(
                    "Autoschematic prefix {} is inside another prefix {}. This is disallowed.",
                    prefix,
                    other_prefix
                )
            }
        }
    }

    Ok(config_file)
}
