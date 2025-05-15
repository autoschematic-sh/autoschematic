
use std::{
    collections::HashMap,
    env::current_dir,
    ffi::CString,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{anyhow, bail};
use autoschematic_core::connector::{
    Connector, ConnectorOp, ConnectorOutbox, GetResourceOutput, OpExecOutput, OpPlanOutput,
    Resource, ResourceAddress,
};

use autoschematic_core::secret::SealedSecret;

use crate::{
    error::{AutoschematicError, AutoschematicErrorType},
    unescape::unescape,
    util::prepare_freethreaded_python_with_venv,
    KEYSTORE,
};
use async_trait::async_trait;
use pyo3::types::PyList;
use pyo3::{exceptions::PyLookupError, prelude::*, types::PyFunction};
use pyo3::{
    pyclass, pyfunction, pymethods, pymodule,
    types::{PyAnyMethods, PyModule, PyModuleMethods, PyTuple},
    wrap_pyfunction, Bound, IntoPyObject, Py, PyAny, PyResult, Python,
};

use regex::Regex;


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

pub struct PythonResourceAddress(PathBuf);
pub struct PythonResource(String);
pub struct PythonConnectorOp(String);


#[async_trait]
impl Connector for PythonConnector {
    async fn new(
        name: &str,
        prefix: &Path,
        outbox: ConnectorOutbox,
    ) -> Result<Box<dyn Connector>, anyhow::Error>
    where
        Self: Sized,
    {
        tracing::debug!("Initializing PythonConnector {} at {:?}", name, prefix);
        // E.G. modules/snowflake.py:SnowflakeConnector
        // TODO load modules/snowflake/connector.yaml and
        //  work it out from there!
        let re = regex::new(r"^(?<path>[^:]+):(?<class>.+)$")?;
        let some(caps) = re.captures(name) else {
            return err(autoschematicerror {
                kind: autoschematicerrortype::invalidconnectorstring(name.to_string()),
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

            let module: Bound<PyModule> =
                PyModule::from_code(py, &module_body, &file_name, &module_name)?;

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
                        Err(anyhow!(
                            "Python connector exception: __init__(): {}",
                            e.to_string()
                        ))
                    }
                }
            }
        })?;

        Ok(Box::new(PythonConnector {
            object: instance,
            outbox: outbox,
        }))
    }

    fn filter(&self, addr: &Path) -> Result<bool, anyhow::Error> {
        let res = Python::with_gil(|py| -> anyhow::Result<bool> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let args = PyTuple::new(py, [addr])?;
            let res = self
                .object
                .call_method1(py, pyo3::intern!(py, "filter"), args)?;

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

            if res.is_truthy(py)? {
                Ok(true)
            } else {
                Ok(false)
            }
        })?;

        Ok(res)
    }

    async fn plan(
        &self,
        addr: &Path,
        current: Option<String>,
        desired: Option<String>,
    ) -> Result<Vec<OpPlanOutput>, anyhow::Error> {
        let ops = Python::with_gil(|py| -> anyhow::Result<Vec<OpPlanOutput>> {
            let logger = LoggingStdout::default().into_pyobject(py)?;
            let sys = py.import("sys")?;
            sys.setattr("stdout", logger)?;

            let addr = addr.to_string_lossy().to_string();
            let args = PyTuple::new(py, [Some(addr), current, desired])?;
            let res = self
                .object
                .call_method1(py, pyo3::intern!(py, "plan"), args);

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
                                })
                            }
                        }
                        Err(e) => {
                            bail!("plan(): failed to downcast result to list. Connector.plan(...) must return a list of OpPlanOutput. {}", e.to_string())
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
                        Err(anyhow!(
                            "Python connector exception: plan(): {}",
                            e.to_string()
                        ))
                    }
                }
            }
        })?;

        // for op in ops {
        //     res.push(Box::new(PythonConnectorOp::from_str(&op)?));
        // }
        Ok(ops)
    }
}