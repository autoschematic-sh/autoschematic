use std::{
    env::{
        consts::{ARCH, OS},
        current_dir,
    },
    path::{Path, PathBuf},
};

use git2::Repository;
use ron::error::SpannedResult;
use serde::{de::DeserializeOwned, Serialize};
use similar::{ChangeTag, TextDiff};

#[cfg(feature = "python")]
use crate::connector::spawn::python::autoschematic_connector_hooks;

use crate::{
    config::AutoschematicConfig,
    diag::{Diagnostic, DiagnosticOutput, DiagnosticPosition, DiagnosticSeverity, DiagnosticSpan},
    error::{AutoschematicError, AutoschematicErrorType},
};

pub use ron::ser::PrettyConfig;

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

lazy_static::lazy_static! {
    pub static ref RON: ron::options::Options = ron::Options::default()
    .with_default_extension(ron::extensions::Extensions::UNWRAP_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::UNWRAP_VARIANT_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::EXPLICIT_STRUCT_NAMES)
    .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
}

pub fn ron_check_eq<T: DeserializeOwned + PartialEq>(a: &str, b: &str) -> Result<bool, anyhow::Error> {
    let Ok(a): SpannedResult<T> = RON.from_str(a) else {
        return Ok(false);
    };
    let Ok(b): SpannedResult<T> = RON.from_str(b) else {
        return Ok(false);
    };
    Ok(a == b)
}

pub fn ron_check_syntax<T: DeserializeOwned>(text: &str) -> Result<DiagnosticOutput, anyhow::Error> {
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

#[cfg(feature = "python")]
pub fn init_pyo3_with_venv(venv_dir: &PathBuf) -> anyhow::Result<()> {
    use std::ffi::CStr;
    use std::mem::size_of;
    use std::ptr::addr_of_mut;

    use libc::wchar_t;
    use pyo3::ffi::*;

    let venv_dir = &venv_dir.to_string_lossy();

    unsafe {
        fn check_exception(env_dir: &str, status: PyStatus, config: &mut PyConfig) -> anyhow::Result<()> {
            unsafe {
                if PyStatus_Exception(status) != 0 {
                    PyConfig_Clear(config);

                    let err_msg = CStr::from_ptr(status.err_msg);

                    anyhow::bail!(
                        "Attempt to init venv at {} failed with exception: {}",
                        env_dir,
                        err_msg.to_str()?
                    )
                } else {
                    Ok(())
                }
            }
        }

        let mut config = std::mem::zeroed::<PyConfig>();
        PyConfig_InitPythonConfig(&mut config);

        config.install_signal_handlers = 0;

        // `wchar_t` is a mess.
        let env_dir_utf16;
        let env_dir_utf32;
        let env_dir_ptr;
        if size_of::<wchar_t>() == size_of::<u16>() {
            env_dir_utf16 = venv_dir.encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
            env_dir_ptr = env_dir_utf16.as_ptr().cast::<wchar_t>();
        } else if size_of::<wchar_t>() == size_of::<u32>() {
            env_dir_utf32 = venv_dir.chars().chain(std::iter::once('\0')).collect::<Vec<_>>();
            env_dir_ptr = env_dir_utf32.as_ptr().cast::<wchar_t>();
        } else {
            anyhow::bail!("unknown encoding for `wchar_t`");
        }

        check_exception(
            venv_dir,
            PyConfig_SetString(addr_of_mut!(config), addr_of_mut!(config.prefix), env_dir_ptr),
            &mut config,
        )?;

        check_exception(venv_dir, Py_InitializeFromConfig(&config), &mut config)?;

        PyConfig_Clear(&mut config);

        PyEval_SaveThread();

        Ok(())
    }
}

#[cfg(feature = "python")]
static START: std::sync::Once = std::sync::Once::new();
#[cfg(feature = "python")]
pub fn prepare_freethreaded_python_with_venv(venv_dir: &PathBuf) {
    use std::process::Command;

    START.call_once_force(|_| unsafe {
        let _output = Command::new("python").arg("-m").arg("venv").arg(&venv_dir).output();
        // Use call_once_force because if initialization panics, it's okay to try again.
        if pyo3::ffi::Py_IsInitialized() == 0 {
            pyo3::append_to_inittab!(autoschematic_connector_hooks);
            let res = init_pyo3_with_venv(venv_dir);
        }
    });
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
        if path.is_absolute() {
            Some(PathBuf::from(path))
        } else {
            None
        }
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
