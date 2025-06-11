use std::path::PathBuf;
use pyo3::prelude::*;
use pyo3::ffi::*;


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

static START: std::sync::Once = std::sync::Once::new();
pub fn prepare_freethreaded_python_with_venv(venv_dir: &PathBuf) {
    use std::process::Command;

    START.call_once_force(|_| unsafe {
        let _output = Command::new("python").arg("-m").arg("venv").arg(&venv_dir).output();
        // Use call_once_force because if initialization panics, it's okay to try again.
        if pyo3::ffi::Py_IsInitialized() == 0 {
            // pyo3::append_to_inittab!(autoschematic_connector_hooks);
            let res = init_pyo3_with_venv(venv_dir);
        }
    });
}
