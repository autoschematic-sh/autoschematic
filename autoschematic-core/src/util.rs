use std::{
    env::{
        consts::{ARCH, OS},
        current_dir,
    },
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use git2::Repository;
use ron::error::SpannedResult;
use serde::{Serialize, de::DeserializeOwned};
use similar::{ChangeTag, TextDiff};

#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg(feature = "python")]
use crate::connector::spawn::python::autoschematic_connector_hooks;

use crate::{
    config::AutoschematicConfig,
    diag::{Diagnostic, DiagnosticOutput, DiagnosticPosition, DiagnosticSeverity, DiagnosticSpan},
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

use std::{ffi::OsString, os::unix::ffi::OsStringExt};
pub fn optional_string_from_utf8(s: Option<OsString>) -> anyhow::Result<Option<String>> {
    match s {
        Some(s) => Ok(Some(String::from_utf8(s.into_vec())?)),
        None => Ok(None),
    }
}

lazy_static::lazy_static! {
    pub static ref RON: ron::options::Options = ron::Options::default()
    .with_default_extension(ron::extensions::Extensions::UNWRAP_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::UNWRAP_VARIANT_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::EXPLICIT_STRUCT_NAMES)
    .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
}

pub fn ron_check_eq<T: DeserializeOwned + PartialEq>(a: &OsStr, b: &OsStr) -> Result<bool, anyhow::Error> {
    let a = str::from_utf8(a.as_bytes())?;
    let b = str::from_utf8(b.as_bytes())?;

    let Ok(a): SpannedResult<T> = RON.from_str(a) else {
        return Ok(false);
    };
    let Ok(b): SpannedResult<T> = RON.from_str(b) else {
        return Ok(false);
    };
    Ok(a == b)
}

pub fn ron_check_syntax<T: DeserializeOwned>(text: &OsStr) -> Result<DiagnosticOutput, anyhow::Error> {
    let text = str::from_utf8(text.as_bytes())?;

    let res = ron::Deserializer::from_str_with_options(text, &*RON);
    match res {
        Ok(mut deserializer) => {
            let result: Result<T, _> = serde_path_to_error::deserialize(&mut deserializer);
            match result {
                Ok(_) => return Ok(DiagnosticOutput::default()),
                Err(e) => {
                    let inner_error = deserializer.span_error(e.inner().clone());
                    return Ok(DiagnosticOutput {
                        diagnostics: vec![Diagnostic {
                            span: DiagnosticSpan {
                                start: DiagnosticPosition {
                                    line: inner_error.position_start.line as u32,
                                    col: inner_error.position_start.col as u32,
                                },
                                end: DiagnosticPosition {
                                    line: inner_error.position_end.line as u32,
                                    col: inner_error.position_end.col as u32,
                                },
                            },
                            severity: DiagnosticSeverity::ERROR as u8,
                            message: format!("{} at {}", inner_error.code, e.path()),
                        }],
                    });
                }
            }
        }
        Err(e) => {
            return Ok(DiagnosticOutput {
                diagnostics: vec![Diagnostic {
                    span: DiagnosticSpan {
                        start: DiagnosticPosition {
                            line: e.position_start.line as u32,
                            col: e.position_start.col as u32,
                        },
                        end: DiagnosticPosition {
                            line: e.position_end.line as u32,
                            col: e.position_end.col as u32,
                        },
                    },
                    severity: DiagnosticSeverity::ERROR as u8,
                    message: format!("{}", e.code),
                }],
            });
        }
    };
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
        lines.push(format!("{}{}", sign, change));
    }
    lines.push(String::from("```\n"));

    return Ok(lines.join(""));
}

pub fn short_target() -> String {
    format!("{}-{}", OS, ARCH)
}

pub fn split_prefix_addr(config: &AutoschematicConfig, path: &Path) -> Option<(PathBuf, PathBuf)> {
    for prefix in config.prefixes.keys() {
        if path.starts_with(prefix) {
            let Ok(addr) = path.strip_prefix(prefix) else {
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
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => return None,
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
