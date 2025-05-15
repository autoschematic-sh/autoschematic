use std::{
    collections::HashMap, fs::File, io::BufWriter, path::{Path, PathBuf}
};

use anyhow::{bail, Context};

use crate::{connector::OutputMap, util::path_relative_from};

pub fn write_virt_output_file(
    virt_output_path: &Path,
    outputs: &OutputMap,
    merge_with_existing: bool,
) -> Result<Option<PathBuf>, anyhow::Error> {
    let mut resulting_hashmap: HashMap<String, String> = HashMap::new();

    if let Some(parent) = virt_output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // If an output file exists, we may merge it with this new output file,
    //  deleting keys where outputs[key].is_none(),
    //  and overwriting keys with the new output values where they collide.
    if virt_output_path.exists() {
        if merge_with_existing {
            let file = std::fs::File::open(&virt_output_path)
                .context(format!("Reading output file {}", virt_output_path.to_string_lossy()))?;
            let reader = std::io::BufReader::new(file);

            let existing_hashmap: HashMap<String, String> = serde_json::from_reader(reader)
                .context(format!("Parsing output file {}", virt_output_path.to_string_lossy()))?;

            for (key, value) in &existing_hashmap {
                if outputs.contains_key(key) {
                    // If the key exists in the new outputs, and is Some(...),
                    //  update the outputs with the new value.
                    if let Some(Some(new_value)) = outputs.get(key) {
                        resulting_hashmap.insert(key.clone(), new_value.clone());
                    }
                    // If None, the value is not propagated into the new hashmap,
                    //  and so is essentially forgotten.
                } else {
                    // Otherwise, retain the original value.
                    resulting_hashmap.insert(key.clone(), value.clone());
                }
            }
            for (key, value) in outputs {
                if let Some(new_value) = value {
                    if !existing_hashmap.contains_key(key) {
                        resulting_hashmap.insert(key.clone(), new_value.clone());
                    }
                }
            }

            if resulting_hashmap.len() == 0 {
                std::fs::remove_file(virt_output_path)?;
                return Ok(None);
            }

            if existing_hashmap == resulting_hashmap {
                return Ok(Some(virt_output_path.into()));
            }
            tracing::debug!("Merging with existing output file at {:?}", virt_output_path);
        } else {
            bail!("Output path at {:?} exists, merge_with_existing = false", virt_output_path)
        }
    } else {
        for (k, v) in outputs {
            if let Some(value) = v {
                resulting_hashmap.insert(k.to_string(), value.to_string());
            }
        }
    }

    let file =
        File::create(&virt_output_path).context(format!("Creating output file {}", virt_output_path.to_string_lossy()))?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, &resulting_hashmap)
        .context(format!("Creating output file {}", virt_output_path.to_string_lossy()))?;

    Ok(Some(virt_output_path.into()))
}

/// To be able to reverse the mapping later, we create a symlink in the repo
/// from the phy addr output path to the virt addr output path.
pub fn link_phy_output_file(
    virt_output_path: &Path,
    phy_output_path: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(phy_parent) = phy_output_path.parent() else {
        bail!("phy_output_path.parent() is None ({:?})", phy_output_path);
    };
    if let Some(rel_path) = path_relative_from(&virt_output_path, &phy_parent) {
        tracing::info!(
            "link_phy_output_file: {:?} -> {:?}, rel_path = {:?}",
            phy_output_path,
            virt_output_path,
            rel_path
        );

        if phy_output_path.is_symlink() {
            if rel_path == std::fs::read_link(&phy_output_path)? {
                return Ok(None);
            }
        }
        if phy_output_path.exists() {
            std::fs::remove_file(&phy_output_path)?;
        }

        std::fs::create_dir_all(phy_parent)?;
        std::os::unix::fs::symlink(&rel_path, &phy_output_path)?;
    } else {
        bail!(
            "Failed to form relative path: {:?} -> {:?}",
            phy_output_path,
            virt_output_path
        );
    }
    Ok(Some(phy_output_path.into()))
}

/// Unlinks an existing phy -> virt output file symlink if it exists.
pub fn unlink_phy_output_file(phy_output_path: &Path) -> anyhow::Result<Option<PathBuf>> {
    // let phy_output_path = build_out_path(prefix, phy_addr);
    // tracing::info!("unlink_phy_output_file: {:?} -> {:?}", phy_addr, virt_addr);

    if phy_output_path.is_symlink() {
        std::fs::remove_file(phy_output_path)?;
        Ok(Some(phy_output_path.into()))
    } else {
        Ok(None)
    }
}
