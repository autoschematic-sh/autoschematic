use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use crate::connector::{OutputMapFile, PhysicalAddress};

// use crate::connector_util::build_out_path;

#[derive(Debug, Serialize, Deserialize, Hash, Clone, Eq, PartialEq)]
pub struct ReadOutput {
    pub addr: PathBuf,
    pub key: String,
}

impl ReadOutput {
    pub fn to_string(&self) -> String {
        format!("out://{}[{}]", self.addr.to_string_lossy(), self.key)
    }
}

// pub type OutputKey = (PathBuf, String);
// pub type ReadOutputSet = HashMap<>;

// For a given resource config definition,
// pull out all of the uses of "out://some_file.ron[key]".
//
pub fn get_read_outputs(config: &str) -> Vec<ReadOutput> {
    // This regex captures:
    // - Group 1: everything after "out://" until the first '[' (the filename)
    // - Group 2: the content between '[' and ']' (the key)
    let re = Regex::new(r#"out://([^\[]+)\[([^\]]+)\]"#).unwrap();
    let mut outputs = Vec::new();
    for cap in re.captures_iter(config) {
        let filename = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let key = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        outputs.push(ReadOutput {
            addr: PathBuf::from(filename),
            key: key.to_string(),
        });
    }
    outputs
}

pub struct TemplateResourceResult {
    pub body: String,
    pub missing: HashSet<ReadOutput>,
}

/// Template a resource body, filling in items of the form
///  out://file_path/etc/etc/other.ron[key]
///  with their actual output values.
pub fn template_config(prefix: &Path, config: &str) -> anyhow::Result<TemplateResourceResult> {
    // Regex to capture the file path and key.
    let re = Regex::new(r#"out://(?<addr>[^\[]+)\[(?<key>[^\]]+)\]"#)?;

    let mut missing = HashSet::<ReadOutput>::new();

    // Replace each occurrence with its fetched value, if available.
    let output = re.replace_all(config, |caps: &Captures| {
        let addr = &caps["addr"];
        let key = &caps["key"];
        let phy_addr = PathBuf::from(addr);
        let addr = PathBuf::from(addr);

        let prefix = prefix.to_path_buf();

        // Attempt to read the output using the provided function.
        // If successful and a value is returned, replace the match.
        match OutputMapFile::get(&prefix, &phy_addr, key) {
            Ok(Some(val)) => val,
            _ => {
                let val = caps.get(0).unwrap().as_str().to_string();
                missing.insert(ReadOutput {
                    addr,
                    key: key.to_string(),
                });
                val
            }
        }
    });

    Ok(TemplateResourceResult {
        body: output.into_owned(),
        missing,
    })
}

// pub fn load_output_ondisk(
//     prefix: &Path,
//     path: &Path,
//     key: String,
// ) -> anyhow::Result<Option<String>> {
//     let path = build_out_path(prefix, path);

//     if !path.is_file() {
//         return Ok(None);
//     }

//     let file = File::open(&path)?;
//     let reader = BufReader::new(file);

//     let existing_hashmap: HashMap<String, String> = serde_json::from_reader(reader)?;

//     Ok(existing_hashmap.get(&key).cloned())
// }
