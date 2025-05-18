use std::{
    ffi::OsString,
    fs::File,
    io::BufReader,
    path::{Component, Path, PathBuf},
};

use anyhow::{bail, Context};
use regex::Regex;

use crate::connector::{OutputMapFile, ResourceAddress};

// TODO Annotate these with custom types so that accidental misuse is impossible
// TODO Add unit tests

pub fn build_out_path(prefix: &Path, addr: &Path) -> PathBuf {
    // Start with prefix
    let mut output = prefix.to_path_buf();

    output.push(".outputs");

    // Join the parent portion of `addr`, if it exists
    if let Some(parent) = addr.parent() {
        // Guard against pathological cases like ".." or "." parents
        // by only pushing normal components
        for comp in parent.components() {
            match comp {
                Component::Normal(_) => output.push(comp),
                _ => {}
            }
        }
    }

    let mut new_filename = OsString::new();
    if let Some(fname) = addr.file_name() {
        new_filename.push(fname);
    } else {
        // If there's no file name at all, we'll just use ".out.json"
        // so `new_filename` right now is just "." — that's fine.
        // We'll end up producing something like "./office/east/ec2/us-east-1/.out.json"
    }
    new_filename.push(".out.json");

    // Final push sets the new filename
    output.push(new_filename);

    // tracing::error!("build_out_path: {:?} / {:?} -> {:?}", prefix, addr, output);
    output
}

pub fn unbuild_out_path(prefix: &Path, addr: &Path) -> anyhow::Result<PathBuf> {
    let new_addr = addr
        .strip_prefix(prefix.join(".outputs"))
        .context(format!("unbuild_out_path({:?}, {:?})", prefix, addr))?;

    let Some(parent) = new_addr.parent() else {
        bail!("unbuild_out_path: bad filename {:?}", addr)
    };

    let Some(filename) = new_addr.file_name() else {
        bail!("unbuild_out_path: bad filename {:?}", addr)
    };

    let Some(filename) = filename.to_str() else {
        bail!("unbuild_out_path: bad filename {:?}", addr)
    };

    let Some(new_filename) = filename.strip_suffix(".out.json") else {
        bail!("unbuild_out_path: bad filename {:?}", addr)
    };

    tracing::error!(
        "unbuild_out_path: {:?} / {:?} -> {:?}",
        prefix,
        addr,
        parent.join(new_filename)
    );

    Ok(parent.join(new_filename))
}

pub fn load_resource_outputs(
    prefix: &Path,
    addr: &impl ResourceAddress,
) -> anyhow::Result<Option<OutputMapFile>> {
    let addr = addr.to_path_buf();
    let output_path = build_out_path(prefix, &addr);

    if output_path.exists() {
        let file = File::open(&output_path)?;
        let reader = BufReader::new(file);

        let existing_hashmap: OutputMapFile = serde_json::from_reader(reader)?;

        Ok(Some(existing_hashmap))
    } else {
        Ok(None)
    }
}

// Given a "physical" addr, if its output file
// is a symlink, reverse it to resolve it to a "virtual" addr.
pub fn output_phy_to_virt<A: ResourceAddress>(
    prefix: &Path,
    addr: &A,
) -> anyhow::Result<Option<A>> {
    let output_path = build_out_path(prefix, &addr.to_path_buf());

    if output_path.exists() {
        if output_path.is_symlink() {
            let Some(parent) = output_path.parent() else {
                bail!("output_path.parent() returned None!")
            };
            let virt_out_path =
                std::fs::canonicalize(parent.join(&std::fs::read_link(&output_path)?))?;
            // HACK ALERT HACK ALERT
            // If we change the assumption that all connectors and commands run from the root of the repository,
            // or if a connector runs cd for some reason, this will break!
            let virt_out_path = virt_out_path.strip_prefix(std::env::current_dir()?)?;
            Ok(A::from_path(&unbuild_out_path(prefix, virt_out_path)?)?)
        } else {
            Ok(Some(addr.clone()))
        }
    } else {
        Ok(Some(addr.clone()))
    }
}

pub fn get_output_or_bail(output_map: &OutputMapFile, key: &str) -> anyhow::Result<String> {
    let Some(output) = &output_map.get(key) else {
        bail!("Couldn't get output key: {}", key)
    };
    Ok(output.to_string())
}

pub fn load_resource_output_key(
    prefix: &Path,
    addr: &impl ResourceAddress,
    key: &str,
) -> anyhow::Result<Option<String>> {
    let Some(outputs) = load_resource_outputs(prefix, addr)? else {
        return Ok(None);
    };

    Ok(outputs.get(key).cloned())
}

pub fn read_mounted_secret(prefix: &Path, secret_ref: &str) -> anyhow::Result<String> {
    let re = Regex::new(r"^secret://(?<path>.+)$")?;

    if let Some(caps) = re.captures(secret_ref) {
        let path = PathBuf::from(&caps["path"]);
        Ok(
            std::fs::read_to_string(PathBuf::from("/tmp/secrets/").join(prefix).join(path))
                .context(format!("Reading secret at ref {}", secret_ref))?,
        )
    } else {
        bail!("read_mounted_secret: invalid ref {}", secret_ref)
    }
}


/// Connectors may save time in list() by avoiding fetching 
/// certain resource types if the subpath argument would filter them out
/// from the results anyway. This is a utility function to check this case.
/// If the subpath select
/// For example: 
/// subpath_filter("aws/s3/us-east-1", "./") -> true
/// subpath_filter("aws/s3/us-east-1", "aws/s3/eu-west-2") -> false
/// subpath_filter("aws/s3/us-east-1", "aws/s3/us-east-1/buckets") -> true
/// subpath_filter("aws/ecs/*/", "aws/s3/us-east-1/buckets") -> true
pub fn subpath_filter(check_path: &Path, subpath: &Path) -> bool {

    true
}