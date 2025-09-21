use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::connector::OutputMapFile;

// use crate::connector_util::build_out_path;

#[derive(Debug, Serialize, Deserialize, Hash, Clone, Eq, PartialEq)]
pub struct ReadOutput {
    pub addr: PathBuf,
    pub key: String,
}

impl ReadOutput {
    pub fn into_string(&self) -> String {
        format!("out://{}[{}]", self.addr.to_string_lossy(), self.key)
    }
}

/// For a given resource config definition,
/// pull out all of the uses of "out://some_file.ron[key]".
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

#[derive(Debug)]
pub struct TemplateResourceResult {
    pub body: String,
    pub found: HashMap<ReadOutput, String>,
    pub missing: HashSet<ReadOutput>,
}

/// Template a resource body, filling in items of the form
///  out://file_path/etc/etc/other.ron[key]
///  with their actual output values.
pub fn template_config(prefix: &Path, config: &str) -> anyhow::Result<TemplateResourceResult> {
    // Regex to capture the file path and key.
    let re = Regex::new(r#"out://(?<addr>[^\[]+)\[(?<key>[^\]]+)\]"#)?;

    let mut found = HashMap::new();
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
            Ok(Some(val)) => {
                found.insert(
                    ReadOutput {
                        addr,
                        key: key.to_string(),
                    },
                    val.clone(),
                );
                val
            }
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
        found,
        missing,
    })
}

/// Given a templated config with "out://resource/address\[key\]" lookups, and a raw config with those raw values,
/// reverse the template operation to produce a version of the raw config with those raw values
/// substituted by equivalent template lookups.
/// Only considers templated values of length > `min_length`.
pub fn reverse_template_config(
    prefix: &Path,
    templated_config: &str,
    raw_config: &str,
    min_length: usize,
) -> anyhow::Result<String> {
    let mut result = raw_config.to_string();
    let template_result = template_config(prefix, templated_config)?;

    for (key, value) in template_result.found {
        if value.len() < min_length {
            continue;
        }

        result = result.replace(&value, &key.into_string());
    }

    Ok(result)
}

const MIN_ANCHOR_LEN: usize = 3;

#[derive(Debug)]
pub struct Comment {
    text: String,
    after: Option<String>,
    before: Option<String>,
}

fn code_lines(target: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut idx = 0;
    for line in target.lines() {
        out.push((idx, line.to_string()));
        idx += line.len() + 1; // +1 for '\n'
    }
    out
}

pub fn extract_comments(src: &str) -> Vec<Comment> {
    let re_line = Regex::new(r"(?m)^\s*//.*").unwrap();

    let mut comments = Vec::new();

    let lines: Vec<&str> = src.lines().collect();

    if let Some(first_line) = lines.first()
        && re_line.is_match(first_line)
    {
        comments.push(Comment {
            text: first_line.to_string(),
            after: lines.get(1).map(|s| s.to_string()),
            before: None,
        });
    }

    for window in lines.windows(3) {
        let prev_line = window
            .first()
            .filter(|s| s.trim().len() >= MIN_ANCHOR_LEN)
            .map(|s| s.to_string());

        let line = window.get(1).unwrap();

        let next_line = window
            .get(2)
            .filter(|s| s.trim().len() >= MIN_ANCHOR_LEN)
            .map(|s| s.to_string());

        if re_line.is_match(line) {
            comments.push(Comment {
                text: line.to_string(),
                after: next_line,
                before: prev_line,
            });
        }
    }

    if let Some(last_line) = lines.last()
        && re_line.is_match(last_line)
    {
        comments.push(Comment {
            text: last_line.to_string(),
            after: None,
            before: lines.get(lines.len() - 2).map(|s| s.to_string()),
        });
    }
    comments
}

use strsim::levenshtein;

pub fn apply_comments(mut target: String, comments: Vec<Comment>) -> String {
    let mut leftover_comments = Vec::new();

    for c in comments {
        // Exact before match
        if let Some(ref anchor) = c.before
            && let Some(pos) = target.find(anchor)
        {
            insert_after_line(&mut target, pos, &c.text);
            continue;
        }

        // Exact after match
        if let Some(ref anchor) = c.after
            && let Some(pos) = target.find(anchor)
        {
            insert_before(&mut target, pos, &c.text);
            continue;
        }

        // Fuzzy before/after match
        if let Some(pos) = fuzzy_find(&code_lines(&target), &c.after, &c.before) {
            insert_before(&mut target, pos, &c.text);
            continue;
        }

        leftover_comments.push(c);

        // // ---------- 4) orphan bucket ----------
        // target.push_str("\n");
        // target.push_str(&c.text);
    }

    for c in leftover_comments {
        // Exact before match
        if let Some(ref anchor) = c.before
            && let Some(pos) = target.find(anchor)
        {
            insert_after_line(&mut target, pos, &c.text);
            continue;
        }

        // Exact after match
        if let Some(ref anchor) = c.after
            && let Some(pos) = target.find(anchor)
        {
            insert_before(&mut target, pos, &c.text);
            continue;
        }

        // Fuzzy before/after match
        if let Some(pos) = fuzzy_find(&code_lines(&target), &c.after, &c.before) {
            insert_before(&mut target, pos, &c.text);
            continue;
        }

        // leftover_comments.push(c);

        // // // ---------- 4) orphan bucket ----------
        // // target.push_str("\n");
        // // target.push_str(&c.text);
    }
    target
}

const MAX_FUZZY_DIST: usize = 6;
fn fuzzy_find(lines: &[(usize, String)], after: &Option<String>, before: &Option<String>) -> Option<usize> {
    // let candidates = before.iter().chain(after.iter());

    for dist in 0..=MAX_FUZZY_DIST {
        // for anchor in candidates.clone() {
        for window in lines.windows(3) {
            match (before, after) {
                (None, None) => {
                    return None;
                }
                (None, Some(after)) => {
                    let (start, _) = window.get(1).unwrap();
                    let (_, next_line) = window.get(2).unwrap();

                    if levenshtein(after.trim(), next_line.trim()) <= dist {
                        return Some(*start);
                    }
                }
                (Some(before), None) => {
                    let (start, _) = window.get(1).unwrap();
                    let (_, prev_line) = window.first().unwrap();

                    if levenshtein(before.trim(), prev_line.trim()) <= dist {
                        return Some(*start);
                    }
                }
                (Some(before), Some(after)) => {
                    let (_, next_line) = window.get(2).unwrap();
                    let (start, _) = window.get(1).unwrap();
                    let (_, prev_line) = window.first().unwrap();

                    if levenshtein(after.trim(), next_line.trim()) <= dist
                        && levenshtein(before.trim(), prev_line.trim()) <= dist
                    {
                        return Some(*start);
                    }
                }
            }
        }
        // }
    }
    None
}

/// Insert comment *before* the anchor.
fn insert_before(buf: &mut String, pos: usize, comment: &str) {
    buf.insert_str(pos, &format!("{comment}\n"));
}

/// Insert comment *after* the entire anchor line.
fn insert_after_line(buf: &mut String, pos: usize, comment: &str) {
    if let Some(newline_off) = buf[pos..].find('\n') {
        let insert_at = pos + newline_off + 1;
        buf.insert_str(insert_at, &format!("{comment}\n"));
    } else {
        // anchor was last line – fallback to before
        insert_before(buf, pos, comment);
    }
}
