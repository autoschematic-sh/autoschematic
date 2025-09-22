use std::{
    collections::HashMap,
    env::{
        consts::{ARCH, OS},
        current_dir,
    },
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use git2::Repository;
use regex::Regex;
use ron;
use ron::error::SpannedResult;
use serde::{Serialize, de::DeserializeOwned};
use similar::{ChangeTag, TextDiff};

// #[cfg(feature = "python")]
// use pyo3::prelude::*;

// #[cfg(feature = "python")]
// use crate::connector::spawn::python::autoschematic_connector_hooks;

use crate::{
    config::AutoschematicConfig,
    diag::{Diagnostic, DiagnosticPosition, DiagnosticResponse, DiagnosticSeverity, DiagnosticSpan},
    error::{AutoschematicError, AutoschematicErrorType},
};

pub use ron::ser::PrettyConfig;

/// Locates the root of a git repository containing the currenty working directory.
/// Fails if the current working directory is not within a git repository.
pub fn repo_root() -> Result<PathBuf, AutoschematicError> {
    let repo = Repository::discover(PathBuf::from("."));
    match repo {
        Ok(repo) => {
            let repo_root = if repo.is_bare() {
                repo.path()
            } else {
                let Some(parent) = repo.path().parent() else {
                    return Err(anyhow::anyhow!("Normal repo, but .git has no parent?").into());
                };
                parent
            };

            Ok(PathBuf::from(repo_root))
        }
        Err(e) => Err(AutoschematicError {
            kind: AutoschematicErrorType::InternalError(anyhow::anyhow!(
                "Couldn't discover a Git repository at: {:?}: {}",
                current_dir()?,
                e
            )),
        }),
    }
}

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

pub fn optional_string_from_utf8(s: Option<Vec<u8>>) -> anyhow::Result<Option<String>> {
    match s {
        Some(s) => Ok(Some(String::from_utf8(s)?)),
        None => Ok(None),
    }
}

// TODO Could this be confusing as hell? This seems non-obvious!
// However, this does mean we can use the exact same config for local and remote...
/// Where no keystore is present, there's no way to unseal a "secret://some/path" reference.
/// Instead, we pass through the corresponding values from environment variables.
pub fn passthrough_secrets_from_env(env: &HashMap<String, String>) -> anyhow::Result<HashMap<String, String>> {
    let re = Regex::new(r"^secret://(?<path>.+)$")?;

    let mut out_map = HashMap::new();

    for (key, value) in env {
        if let Some(_caps) = re.captures(value) {
            if let Ok(env_value) = std::env::var(key) {
                out_map.insert(key.to_string(), env_value);
            }
        } else {
            out_map.insert(key.to_string(), value.to_string());
        }
    }

    Ok(out_map)
}

lazy_static::lazy_static! {
    pub static ref RON: ron::options::Options = ron::Options::default()
    .with_default_extension(ron::extensions::Extensions::UNWRAP_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::UNWRAP_VARIANT_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::EXPLICIT_STRUCT_NAMES)
    .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
}

pub fn ron_check_eq<T: DeserializeOwned + PartialEq>(a: &[u8], b: &[u8]) -> Result<bool, anyhow::Error> {
    let a = std::str::from_utf8(a)?;
    let b = std::str::from_utf8(b)?;

    let Ok(a): SpannedResult<T> = RON.from_str(a) else {
        return Ok(false);
    };
    let Ok(b): SpannedResult<T> = RON.from_str(b) else {
        return Ok(false);
    };
    Ok(a == b)
}

pub fn ron_check_syntax<T: DeserializeOwned>(text: &[u8]) -> Result<Option<DiagnosticResponse>, anyhow::Error> {
    let text = std::str::from_utf8(text)?;

    let res = ron::Deserializer::from_str_with_options(text, &RON);
    match res {
        Ok(mut deserializer) => {
            let result: Result<T, _> = serde_path_to_error::deserialize(&mut deserializer);
            match result {
                Ok(_) => Ok(None),
                Err(e) => {
                    let inner_error = deserializer.span_error(e.inner().clone());
                    Ok(Some(DiagnosticResponse {
                        diagnostics: vec![Diagnostic {
                            span: DiagnosticSpan {
                                start: DiagnosticPosition {
                                    line: inner_error.span.start.line as u32,
                                    col: inner_error.span.start.col as u32,
                                },
                                end: DiagnosticPosition {
                                    line: inner_error.span.end.line as u32,
                                    col: inner_error.span.end.col as u32,
                                },
                            },
                            severity: DiagnosticSeverity::ERROR as u8,
                            message: format!("{} at {}", inner_error.code, e.path()),
                        }],
                    }))
                }
            }
        }
        Err(e) => Ok(Some(DiagnosticResponse {
            diagnostics: vec![Diagnostic {
                span: DiagnosticSpan {
                    start: DiagnosticPosition {
                        line: e.span.start.line as u32,
                        col: e.span.start.col as u32,
                    },
                    end: DiagnosticPosition {
                        line: e.span.end.line as u32,
                        col: e.span.end.col as u32,
                    },
                },
                severity: DiagnosticSeverity::ERROR as u8,
                message: format!("{}", e.code),
            }],
        })),
    }
}

pub fn diff_ron_values<T>(a: &T, b: &T) -> anyhow::Result<String>
where
    T: Serialize,
{
    let pretty_config = PrettyConfig::default().struct_names(true);

    let a_s = RON.to_string_pretty(a, pretty_config.clone())?;
    let b_s = RON.to_string_pretty(b, pretty_config.clone())?;

    let diff = TextDiff::from_lines(&a_s, &b_s);

    let mut lines = Vec::<String>::new();

    lines.push(String::from("```diff\n"));

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        lines.push(format!("{sign}{change}"));
    }
    lines.push(String::from("```\n"));

    Ok(lines.join(""))
}

pub fn short_target() -> String {
    format!("{OS}-{ARCH}")
}

pub fn split_prefix_addr(config: &AutoschematicConfig, path: &Path) -> Option<(PathBuf, PathBuf)> {
    // Some additional tweaks to make it easier on users:
    // The "/" prefix is equivalent to ""
    // The "/example" and "./example" prefixes are equivalent to "example/"
    for prefix in config.prefixes.keys() {
        let norm_prefix = prefix.strip_suffix("/").unwrap_or(prefix);

        let norm_prefix = norm_prefix.strip_prefix("/").unwrap_or(norm_prefix);

        let norm_prefix = norm_prefix.strip_prefix("./").unwrap_or(norm_prefix);

        // let norm_prefix = if norm_prefix == "" { "/" } else { norm_prefix };

        if path.starts_with(norm_prefix) {
            let Ok(addr) = path.strip_prefix(norm_prefix) else {
                return None;
            };
            return Some((PathBuf::from(prefix), addr.into()));
        }
    }
    None
}

// This routine is adapted from the *old* Path's `path_relative_from`
// function, which works differently from the new `relative_from` function.
// In particular, this handles the case on unix where both paths are
// absolute but with only the root as the common directory.
pub fn path_relative_from(path: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() { Some(PathBuf::from(path)) } else { None }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(Component::CurDir)) => comps.push(a),
                (Some(_), Some(Component::ParentDir)) => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }
        Some(comps.iter().map(|c| c.as_os_str()).collect())
    }
}

pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;

        let dst_path = dst.as_ref().join(entry.file_name());

        if ty.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir_all(entry.path(), dst_path)?;
        } else {
            // eprintln!("COPY {:?} -> {:?}", entry.path(), dst_path);
            // tracing::error!("COPY {:?} -> {:?}", entry.path(), dst_path);
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
