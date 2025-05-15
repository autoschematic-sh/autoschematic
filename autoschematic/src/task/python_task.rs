use std::{
    env::current_dir,
    ffi::CString,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use autoschematic_core::{unescape, util::prepare_freethreaded_python_with_venv};
use octocrab::{models::InstallationId, Octocrab};
use pyo3::{
    pyclass, pymethods,
    types::{PyAnyMethods, PyList, PyListMethods, PyModule, PyTuple},
    Bound, IntoPyObject, Py, PyAny, Python,
};
use rand::{distr::Alphanumeric, Rng};
use regex::Regex;
use secrecy::SecretBox;
use tempdir::TempDir;

use crate::{
    chwd::ChangeWorkingDirectory,
    credentials,
    error::{AutoschematicServerError, AutoschematicServerErrorType},
    git_util::{checkout_new_branch, clone_repo, git_add, git_commit_and_push, pull_with_rebase},
    github_util::create_pull_request,
};

use super::{
    message::TaskMessage,
    state::{self, TaskState},
    util::{drain_inbox, wait_for_comment_types},
    Task, TaskInbox, TaskOutbox,
};

mod util;

pub struct PythonTask {
    pub owner: String,
    pub repo: String,
    prefix: PathBuf,
    pub class_name: String,
    temp_dir: TempDir,
    token: SecretBox<str>,
    pub client: Octocrab,
    object: Py<PyAny>,
    inbox: TaskInbox,
    outbox: TaskOutbox,
}
//
// Safety note: Must hold GIL when operating!
unsafe impl Send for PythonTask {}
unsafe impl Sync for PythonTask {}

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

#[async_trait]
impl Task for PythonTask {
    async fn new(
        owner: &str,
        repo: &str,
        prefix: &Path,
        name: &str,
        inbox: TaskInbox,
        outbox: TaskOutbox,
        installation_id: u64,
    ) -> Result<Box<dyn Task>, anyhow::Error>
    where
        Self: Sized,
    {
        let (client, token) =
            credentials::octocrab_installation_client(InstallationId(installation_id)).await?;

        outbox
            .send(TaskMessage::StateChange(state::TaskState::Running))
            .await?;
        // let re = Regex::new(r"^(?<type>[^:]+):(?<path>.+)$")?;
        let re = Regex::new(r"^(?<path>[^:]+):(?<class>.+)$")?;
        let Some(caps) = re.captures(name) else {
            return Err(AutoschematicServerError {
                kind: AutoschematicServerErrorType::InvalidConnectorString(name.to_string()),
            }
            .into());
        };

        let module_path = PathBuf::from(&caps["path"]);
        let Some(module_parent) = module_path.parent() else {
            bail!("Failed to get module parent directory: {}", &caps["path"]);
        };

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

        let (instance, log_lines): (Py<PyAny>, String) =
            Python::with_gil(|py| -> anyhow::Result<(Py<PyAny>, String)> {
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
                let output_lines = match output {
                    Ok(lines) => {
                        if let Ok(lines) = lines.extract::<String>() {
                            lines
                            // if lines.len() > 0 {
                            //     outbox.send(TaskMessage::LogLines(lines)).await;
                            // }
                        } else {
                            String::new()
                        }
                    }
                    Err(_) => String::new(),
                };

                match instance {
                    Ok(i) => Ok((i.into(), output_lines)),
                    Err(e) => {
                        if let Some(traceback) = e.traceback(py) {
                            let format_tb =
                                PyModule::import(py, "traceback")?.getattr("format_tb")?;

                            let args = PyTuple::new(py, [traceback])?;
                            let full_traceback = format!("{}", format_tb.call1(args)?);
                            Err(anyhow!(
                                "Python task exception: __init__(): {}: Traceback: {}",
                                e.to_string(),
                                unescape::unescape(&full_traceback).unwrap()
                            ))
                        } else {
                            Err(anyhow!(
                                "Python task exception: __init__(): {}",
                                e.to_string()
                            ))
                        }
                    }
                }
            })?;

        if log_lines.len() > 0 {
            outbox.send(TaskMessage::LogLines(log_lines)).await?;
        }

        Ok(Box::new(PythonTask {
            owner: owner.into(),
            repo: repo.into(),
            prefix: prefix.into(),
            temp_dir: TempDir::new("autoschematic_task")?,
            class_name: class_name.into(),
            object: instance,
            token,
            client,
            inbox,
            outbox,
        }))
    }

    async fn run(mut self: Box<Self>, arg: serde_json::Value) -> anyhow::Result<()> {
        self.outbox
            .send(TaskMessage::StateChange(TaskState::Running))
            .await?;

        let _ = drain_inbox(&mut self.inbox).await.map_err(async |e| {
            tracing::error!("{}", e);
            let _ = self
                .outbox
                .send(TaskMessage::StateChange(TaskState::Stopped))
                .await;
        });

        let repo = self.client.repos(&self.owner, &self.repo).get().await?;

        let Some(default_branch) = repo.default_branch else {
            bail!("Repo {}/{} has no default branch", self.owner, self.repo)
        };

        // let head_ref = self
        //     .client
        //     .repos(&self.owner, &self.repo)
        //     .get_ref(&octocrab::params::repos::Reference::Branch(
        //         default_branch.clone(),
        //     ))
        //     .await?;
        clone_repo(
            &self.owner,
            &self.repo,
            self.temp_dir.path(),
            &default_branch,
            &self.token,
        )
        .await
        .context("Cloning repo")?;

        let _ = drain_inbox(&mut self.inbox).await.map_err(async |e| {
            tracing::error!("{}", e);
            let _ = self
                .outbox
                .send(TaskMessage::StateChange(TaskState::Stopped))
                .await;
        });

        let rand_suffix: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(20)
            .map(char::from)
            .collect();

        let branch_name = format!("task-py/{}-{}", self.class_name, rand_suffix);

        let repo_path = self.repo_path();

        let _chwd = ChangeWorkingDirectory::change(&repo_path)?;

        let prefix = if self.prefix.is_absolute() {
            self.prefix.strip_prefix("/")?
        } else {
            &self.prefix
        };

        checkout_new_branch(&repo_path, &branch_name)
            .await
            .context("Checking out branch")?;

        git_commit_and_push(
            &repo_path,
            &branch_name,
            &self.token,
            &format!("Create branch {}", branch_name),
        )
        .context("git_commit_and_push")?;

        let issue_number = create_pull_request(
            &self.owner,
            &self.repo,
            &branch_name,
            &branch_name,
            "main",
            &self.client,
        )
        .await
        .context("Creating pull request")?;

        wait_for_comment_types(
            &self.owner.clone(),
            &self.repo.clone(),
            issue_number,
            &["greeting"],
            &mut self.inbox,
        )
        .await?;

        let (file_list, log_lines) =
            Python::with_gil(|py| -> anyhow::Result<(Vec<PathBuf>, String)> {
                let logger = LoggingStdout::default().into_pyobject(py)?;
                let sys = py.import("sys")?;
                sys.setattr("stdout", logger)?;

                let args = PyTuple::new(py, [prefix.to_str().unwrap(), &arg.to_string()])?;
                let res = self
                    .object
                    .call_method1(py, pyo3::intern!(py, "run"), args)?;

                let output = sys.getattr("stdout")?.getattr("lines");
                let log_lines = match output {
                    Ok(lines) => {
                        if let Ok(lines) = lines.extract::<String>() {
                            lines
                            // if lines.len() > 0 {
                            //     self.outbox.send(TaskMessage::LogLines(lines)).await;
                            // }
                        } else {
                            String::new()
                        }
                    }
                    Err(_) => String::new(),
                };

                let Ok(file_list) = res.downcast_bound::<PyList>(py) else {
                    bail!("Failed to downcast PythonTask::run() result to list");
                };

                let file_list = file_list
                    .iter()
                    .map(|v| PathBuf::from(v.to_string()))
                    .collect();

                Ok((file_list, log_lines))
            })?;

        if log_lines.len() > 0 {
            self.outbox.send(TaskMessage::LogLines(log_lines)).await?;
        }

        for path in &file_list {
            git_add(&repo_path, path)?;
        }

        git_commit_and_push(
            &repo_path,
            &branch_name,
            &self.token,
            &format!("op-python: {}", self.class_name),
        )?;

        let comment_type = self.plan(issue_number).await?;

        loop {
            let mut do_apply = false;
            let mut have_deferrals = false;
            match comment_type.as_str() {
                "plan_overall_success" => {
                    do_apply = true;
                }
                "plan_overall_success_with_deferrals" => {
                    have_deferrals = true;
                    do_apply = true;
                }
                "plan_no_changes" => {}
                "filter_matched_no_files" => {}
                "plan_overall_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                "plan_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                "misc_error" => {
                    bail!("Plan threw an error. Quitting!")
                }
                t => {
                    bail!("unexpected message type {}", t)
                }
            }

            if do_apply {
                let comment_type = self.apply(issue_number).await?;
                tracing::warn!("Apply type: {}", comment_type);

                match comment_type.as_str() {
                    "apply_overall_success" => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        pull_with_rebase(&repo_path, &branch_name, &self.token)?;

                        if have_deferrals {
                            continue;
                        } else {
                            break;
                        }
                    }
                    "apply_success" => {}
                    "apply_error" => {
                        bail!("Apply threw an error. Quitting!")
                    }
                    "misc_error" => {
                        bail!("Apply threw an error. Quitting!")
                    }
                    t => {
                        bail!("unexpected message type {}", t)
                    }
                }
            } else {
                break;
            }
        }

        Ok(())
    }
}

impl PythonTask {
    pub fn repo_path(&self) -> PathBuf {
        self.temp_dir.path().join(&self.owner).join(&self.repo)
    }
}
