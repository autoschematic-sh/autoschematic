use std::{
    collections::HashMap,
    env::current_dir,
    ffi::{CString, OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{anyhow, bail};
use autoschematic_core::{
    connector::{
        Connector, ConnectorOp, ConnectorOutbox, DocIdent, FilterOutput, GetDocOutput, GetResourceOutput, OpExecOutput,
        OpPlanOutput, Resource, ResourceAddress,
    },
    diag::DiagnosticOutput,
    util::optional_string_from_utf8,
};

use async_trait::async_trait;
use autoschematic_core::{
    error::{AutoschematicError, AutoschematicErrorType},
    unescape::unescape,
};
use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::{
    Bound, IntoPyObject, Py, PyAny, PyResult, Python, pyclass, pymethods, pymodule,
    types::{PyAnyMethods, PyModule, PyTuple},
};

use regex::Regex;
use tokio::sync::mpsc::Receiver;

use crate::util::prepare_freethreaded_python_with_venv;

pub struct PythonConnector {
    object: Py<PyAny>,
    outbox: ConnectorOutbox,
}

// Safety note: Must hold GIL when operating!
unsafe impl Send for PythonConnector {}
unsafe impl Sync for PythonConnector {}

///This class is used to hook the Python connector's stdout and capture
///it for display.
#[derive(Default, Clone)]
#[pyclass]
struct LoggingStdout {
    #[pyo3(get)]
    pub lines: String,
}

#[pymethods]
impl LoggingStdout {
    fn write(&mut self, data: &str) {
        tracing::info!("Python stdout: {}", data);
        self.lines.push_str(data);
    }
}

// #[pyfunction]
// fn read_secret(path: &str) -> PyResult<String> {
//     let seal_str = std::fs::read_to_string(PathBuf::from(path))?;
//     let seals: Vec<SealedSecret> = serde_json::from_str(&seal_str).unwrap();

//     for seal in seals {
//         let value = KEYSTORE.get().unwrap().unseal_secret(&seal).unwrap();
//         return Ok(value).into();
//     }

//     Err(PyLookupError::new_err(
//         "No key was found in the keystore that could unseal this secret",
//     ))
// }

// #[pyfunction]
// fn read_output(path: &str) -> PyResult<String> {
//     let seal_str = std::fs::read_to_string(PathBuf::from(path))?;
//     let seals: Vec<SealedSecret> = serde_json::from_str(&seal_str).unwrap();

//     for seal in seals {
//         let value = KEYSTORE.get().unwrap().unseal_secret(&seal).unwrap();
//         return Ok(value).into();
//     }

//     Err(PyLookupError::new_err(
//         "No key was found in the keystore that could unseal this secret",
//     ))
// }

#[pymodule]
pub fn autoschematic_connector_hooks(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // 1) Add our `read_secret` function to this module so Python can see it if needed:
    // m.add_function(wrap_pyfunction!(read_secret, m)?)?;

    // // 2) Import the Python module we want to patch:
    // let autoschematic_connector = m.py().import("autoschematic_connector")?;

    // // 3) Override its `read_secret` function with our Rust implementation:
    // let rust_read_secret = m.getattr("read_secret")?;
    // autoschematic_connector.setattr("read_secret", rust_read_secret)?;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct PythonResourceAddress(PathBuf);
pub struct PythonResource(String);

#[derive(Debug, Clone)]
pub struct PythonConnectorOp(String);

impl ResourceAddress for PythonResourceAddress {
    fn to_path_buf(&self) -> PathBuf {
        self.0.to_path_buf()
    }

    fn from_path(path: &Path) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        Ok(PythonResourceAddress(path.to_path_buf()))
    }
}

impl Resource for PythonResource {
    fn to_os_string(&self) -> Result<OsString, anyhow::Error> {
        Ok(self.0.to_owned().into())
    }

    fn from_os_str(_addr: &impl ResourceAddress, s: &OsStr) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        let s = str::from_utf8(s.as_bytes())?;
        Ok(PythonResource(s.to_string()))
    }
}

impl ConnectorOp for PythonConnectorOp {
    fn to_string(&self) -> Result<String, anyhow::Error> {
        Ok(self.0.to_owned())
    }

    fn from_str(s: &str) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        Ok(PythonConnectorOp(s.to_string()))
    }
}

#[async_trait]
impl Connector for PythonConnector {
    async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> Result<Box<dyn Connector>, anyhow::Error>
    where
        Self: Sized,
    {
        tracing::debug!("Initializing PythonConnector {} at {:?}", name, prefix);
        // E.G. modules/snowflake.py:SnowflakeConnector
        // TODO load modules/snowflake/connector.yaml and
        //  work it out from there!
        let re = Regex::new(r"^(?<path>[^:]+):(?<class>.+)$")?;
        let Some(caps) = re.captures(name) else {
            return Err(AutoschematicError {
                kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
            }
            .into());
        };

        let module_path = PathBuf::from(&caps["path"]);
        let Some(module_parent) = module_path.parent() else {
            bail!("Failed to get module parent directory: {}", &caps["path"]);
        };

        // let venv_dir = current_dir()?.join(module_parent).join(".venv");
        let venv_dir = current_dir()?.join(".venv");

        let module_body = std::fs::read_to_string(&caps["path"])?;
        let class_name = &caps["class"];

        let module_body = CString::from_str(&module_body)?;
        let module_name = CString::from_str(&String::new())?;
        let file_name = CString::from_str(&String::new())?;

        prepare_freethreaded_python_with_venv(&venv_dir);

        if module_parent.join("requirements.txt").is_file() {
            let req_path = module_parent.join("requirements.txt");

            let _res = Python::with_gil(|py| -> anyhow::Result<()> {
                let args = PyTuple::new(py, [["install", "-r", &req_path.to_string_lossy()]])?;
                // let args = PyTuple::new(py, args)?;
                let pip = py.import("pip")?;
                pip.call_method1("main", args)?;
                Ok(())
            })?;
        }

        let instance: Py<PyAny> = Python::with_gil(|py| -> anyhow::Result<Py<PyAny>> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let module: Bound<PyModule> = PyModule::from_code(py, &module_body, &file_name, &module_name)?;

            let class = module.getattr(class_name)?;
            let args = PyTuple::new(py, [prefix])?;
            let instance = class.call(args, None);

            // Dump the stdout from the python process
            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            match instance {
                Ok(i) => Ok(i.into()),
                Err(e) => {
                    if let Some(traceback) = e.traceback(py) {
                        let format_tb = PyModule::import(py, "traceback")?.getattr("format_tb")?;

                        let args = PyTuple::new(py, [traceback])?;
                        let full_traceback = format!("{}", format_tb.call1(args)?);
                        Err(anyhow!(
                            "Python connector exception: __init__(): {}: Traceback: {}",
                            e.to_string(),
                            unescape(&full_traceback).unwrap()
                        ))
                    } else {
                        Err(anyhow!("Python connector exception: __init__(): {}", e.to_string()))
                    }
                }
            }
        })?;

        Ok(Box::new(PythonConnector {
            object: instance,
            outbox: outbox,
        }))
    }

    async fn init(&self) -> Result<(), anyhow::Error> {
        let res = Python::with_gil(|py| -> anyhow::Result<()> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            // let args = PyTuple::new(py, [])?;
            let res = self.object.call_method0(py, pyo3::intern!(py, "init"))?;

            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            self.outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            Ok(())
        })?;

        Ok(res)
    }

    async fn filter(&self, addr: &Path) -> Result<FilterOutput, anyhow::Error> {
        let res = Python::with_gil(|py| -> anyhow::Result<String> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let args = PyTuple::new(py, [addr])?;
            let res = self.object.call_method1(py, pyo3::intern!(py, "filter"), args)?;

            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            self.outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            Ok(res.to_string())
        })?;

        match res.as_str() {
            "Resource" => Ok(FilterOutput::Resource),
            "Config" => Ok(FilterOutput::Config),
            _ => Ok(FilterOutput::None),
        }
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<OsString>,
        desired: Option<OsString>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error> {
        let ops = Python::with_gil(|py| -> anyhow::Result<Vec<OpPlanOutput>> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let current = optional_string_from_utf8(current)?;
            let desired = optional_string_from_utf8(desired)?;

            let addr = addr.to_string_lossy().to_string();
            let args = PyTuple::new(py, [Some(addr), current, desired])?;
            let res = self.object.call_method1(py, pyo3::intern!(py, "plan"), args);

            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            self.outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            match res {
                Ok(r) => {
                    let mut ops = Vec::new();

                    match r.downcast_bound::<PyList>(py) {
                        Ok(op_outputs) => {
                            for op in op_outputs {
                                let op_definition = op.getattr("op_definition")?.extract()?;
                                let friendly_message = match op.getattr("friendly_message") {
                                    Ok(msg) => Some(msg.extract()?),
                                    Err(_) => None, //friendly_message is optional, so skip
                                };
                                ops.push(OpPlanOutput {
                                    op_definition,
                                    friendly_message,
                                    writes_outputs: Vec::new(),
                                })
                            }
                        }
                        Err(e) => {
                            bail!(
                                "plan(): failed to downcast result to list. Connector.plan(...) must return a list of OpPlanOutput. {}",
                                e.to_string()
                            )
                        }
                    };

                    Ok(ops)
                }
                Err(e) => {
                    if let Some(traceback) = e.traceback(py) {
                        let format_tb = PyModule::import(py, "traceback")?.getattr("format_tb")?;

                        let args = PyTuple::new(py, [traceback])?;
                        let full_traceback = format!("{}", format_tb.call1(args)?);
                        Err(anyhow!(
                            "Python connector exception: plan(): {}: Traceback: {}",
                            e.to_string(),
                            unescape(&full_traceback).unwrap()
                        ))
                    } else {
                        Err(anyhow!("Python connector exception: plan(): {}", e.to_string()))
                    }
                }
            }
        })?;

        // for op in ops {
        //     res.push(Box::new(PythonConnectorOp::from_str(&op)?));
        // }
        Ok(ops)
    }

    async fn op_exec(&self, addr: &Path, op: &str) -> Result<OpExecOutput, anyhow::Error> {
        let res = Python::with_gil(|py| -> anyhow::Result<OpExecOutput> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let addr = addr.to_string_lossy();
            let args = PyTuple::new(py, [&addr, op])?;
            let res = self.object.call_method1(py, pyo3::intern!(py, "op_exec"), args);

            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            self.outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            match res {
                Ok(r) => {
                    let output = OpExecOutput {
                        outputs: r.getattr(py, "outputs")?.extract(py)?,
                        friendly_message: match r.getattr(py, "friendly_message") {
                            Ok(py_friendly_message) => Some(py_friendly_message.extract(py)?),
                            Err(_) => None,
                        },
                    };
                    Ok(output)
                }
                Err(e) => {
                    if let Some(traceback) = e.traceback(py) {
                        let format_tb = PyModule::import(py, "traceback")?.getattr("format_tb")?;

                        let args = PyTuple::new(py, [traceback])?;
                        let full_traceback = format!("{}", format_tb.call1(args)?);
                        Err(anyhow!(
                            "Python connector exception: op_exec(): {}: Traceback: {}",
                            e.to_string(),
                            unescape(&full_traceback).unwrap()
                        ))
                    } else {
                        Err(anyhow!("Python connector exception: op_exec(): {}", e.to_string()))
                    }
                }
            }
        })?;

        Ok(res)
    }

    async fn get(&self, addr: &Path) -> Result<Option<GetResourceOutput>, anyhow::Error> {
        let res = Python::with_gil(|py| -> Result<Option<GetResourceOutput>, anyhow::Error> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let args = PyTuple::new(py, [addr])?;
            let res = self.object.call_method1(py, pyo3::intern!(py, "get"), args);

            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            self.outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            match res {
                Ok(r) => {
                    if r.is_truthy(py)? {
                        let outputs: Option<HashMap<String, Option<String>>> = match r.getattr(py, "outputs") {
                            Ok(val) => val.extract(py)?,
                            Err(_) => None,
                        };
                        let resource_definition: String = r.getattr(py, "resource_definition")?.extract(py)?;
                        // Ok(Some(r.extract(py)?))
                        Ok(Some(GetResourceOutput {
                            outputs: outputs,
                            resource_definition: resource_definition.into(),
                        }))
                    } else {
                        Ok(None)
                    }
                }
                Err(e) => {
                    if let Some(traceback) = e.traceback(py) {
                        let format_tb = PyModule::import(py, "traceback")?.getattr("format_tb")?;

                        let args = PyTuple::new(py, [traceback])?;
                        let full_traceback = format!("{}", format_tb.call1(args)?);
                        Err(anyhow!(
                            "Python connector exception: get(): {}: Traceback: {}",
                            e.to_string(),
                            unescape(&full_traceback).unwrap()
                        ))
                    } else {
                        Err(anyhow!("Python connector exception: get(): {}", e.to_string()))
                    }
                }
            }
        })?;

        Ok(res)
    }

    async fn list(&self, subpath: &Path) -> Result<Vec<PathBuf>, anyhow::Error> {
        let res = Python::with_gil(|py| -> anyhow::Result<Vec<PathBuf>> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let args = PyTuple::new(py, [subpath])?;
            let res = self.object.call_method1(py, pyo3::intern!(py, "list"), args);

            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            self.outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            match res {
                Ok(r) => Ok(r.extract(py)?),
                Err(e) => {
                    if let Some(traceback) = e.traceback(py) {
                        let format_tb = PyModule::import(py, "traceback")?.getattr("format_tb")?;

                        let args = PyTuple::new(py, [traceback])?;
                        let full_traceback = format!("{}", format_tb.call1(args)?);
                        Err(anyhow!(
                            "Python connector exception: list(): {}: Traceback: {}",
                            e.to_string(),
                            unescape(&full_traceback).unwrap()
                        ))
                    } else {
                        Err(anyhow!("Python connector exception: list(): {}", e.to_string()))
                    }
                }
            }
        });

        Ok(res?)
    }

    async fn eq(&self, addr: &Path, a: &OsStr, b: &OsStr) -> anyhow::Result<bool> {
        let res = Python::with_gil(|py| -> anyhow::Result<bool> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let a = str::from_utf8(a.as_bytes())?;
            let b = str::from_utf8(b.as_bytes())?;

            let args = PyTuple::new(py, [addr.to_string_lossy().into(), a.to_string(), b.to_string()])?;
            let res = self.object.call_method1(py, pyo3::intern!(py, "eq"), args);

            let output = sys.getattr("stdout")?.getattr("lines");
            match output {
                Ok(lines) => {
                    if let Ok(lines) = lines.extract::<String>() {
                        if lines.len() > 0 {
                            self.outbox.send(Some(lines))?;
                        }
                    }
                }
                Err(_) => {}
            }

            match res {
                Ok(r) => Ok(r.extract(py)?),
                Err(e) => {
                    if let Some(traceback) = e.traceback(py) {
                        let format_tb = PyModule::import(py, "traceback")?.getattr("format_tb")?;

                        let args = PyTuple::new(py, [traceback])?;
                        let full_traceback = format!("{}", format_tb.call1(args)?);
                        Err(anyhow!(
                            "Python connector exception: list(): {}: Traceback: {}",
                            e.to_string(),
                            unescape(&full_traceback).unwrap()
                        ))
                    } else {
                        Err(anyhow!("Python connector exception: list(): {}", e.to_string()))
                    }
                }
            }
        })?;
        Ok(res)
    }

    async fn diag(&self, addr: &Path, a: &OsStr) -> anyhow::Result<DiagnosticOutput> {
        // TODO re-enable diag!
        Ok(DiagnosticOutput::default())
        // let res = Python::with_gil(|py| -> anyhow::Result<DiagnosticOutput> {
        //     let logger = LoggingStdout::default().into_pyobject(py)?;
        //     let sys = py.import("sys")?;
        //     sys.setattr("stdout", logger)?;

        //     let a = str::from_utf8(a.as_bytes())?;

        //     let args = PyTuple::new(py, [addr.to_string_lossy().into(), a.to_string()])?;
        //     let res = self.object.call_method1(py, pyo3::intern!(py, "diag"), args);

        //     let output = sys.getattr("stdout")?.getattr("lines");
        //     match output {
        //         Ok(lines) => {
        //             if let Ok(lines) = lines.extract::<String>() {
        //                 if lines.len() > 0 {
        //                     self.outbox.send(Some(lines))?;
        //                 }
        //             }
        //         }
        //         Err(_) => {}
        //     }

        //     match res {
        //         Ok(r) => Ok(r.extract(py)?),
        //         Err(e) => {
        //             if let Some(traceback) = e.traceback(py) {
        //                 let format_tb = PyModule::import(py, "traceback")?.getattr("format_tb")?;

        //                 let args = PyTuple::new(py, [traceback])?;
        //                 let full_traceback = format!("{}", format_tb.call1(args)?);
        //                 Err(anyhow!(
        //                     "Python connector exception: list(): {}: Traceback: {}",
        //                     e.to_string(),
        //                     unescape(&full_traceback).unwrap()
        //                 ))
        //             } else {
        //                 Err(anyhow!("Python connector exception: list(): {}", e.to_string()))
        //             }
        //         }
        //     }
        // })?;
        // Ok(res)
    }
}
