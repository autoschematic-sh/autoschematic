pub mod test_task;

use std::{path::Path, sync::OnceLock};

use anyhow::{Context, bail};
use autoschematic_core::{
    aux_task::{
        Task,
        message::TaskMessage,
        registry::{TaskRegistry, TaskRegistryEntry, TaskRegistryKey},
        state::TaskState,
    },
    error::{AutoschematicError, AutoschematicErrorType},
};
use regex::Regex;

use crate::{aux_task::test_task::TestTask, safety_lock::check_safety_lock};

pub static TASK_REGISTRY: OnceLock<TaskRegistry> = OnceLock::new();

pub async fn spawn_task(
    owner: &str,
    repo: &str,
    prefix: &Path,
    name: &str,
    installation_id: u64,
    arg: serde_json::Value,
    blocking: bool,
) -> anyhow::Result<()> {
    check_safety_lock()?;

    // Match a Task name.
    // Task names take the form:
    // {type}:{path}
    // Where task implementations may further interpret `path`
    // for other functionality.
    // E.G. test:fuzz/aws/iam
    let re = Regex::new(r"^(?<type>[^:]+):(?<path>.+)$")?;
    let Some(caps) = re.captures(name) else {
        return Err(AutoschematicError {
            kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
        }
        .into());
    };

    let registry_key = TaskRegistryKey {
        owner: owner.into(),
        repo: repo.into(),
        prefix: prefix.into(),
        task_name: name.into(),
    };

    let (registry_outbox, task_inbox) = tokio::sync::mpsc::channel(64);
    let (task_outbox, mut registry_inbox) = tokio::sync::mpsc::channel(64);

    let (_dummy_send, registry_broadcast) = tokio::sync::broadcast::channel(64);

    let broadcast_registry_key = registry_key.clone();
    let _broadcast_handle: tokio::task::JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        loop {
            let res = registry_inbox.recv().await;
            match res {
                Some(msg) => {
                    // tracing::info!("Got Message from task: {:?}", msg);
                    match msg {
                        TaskMessage::StateChange(ref value) => {
                            if let Some(registry) = TASK_REGISTRY.get() {
                                let mut registry = registry.entries.write().await;
                                if let Some(entry) = registry.get_mut(&broadcast_registry_key) {
                                    entry.state = value.clone();
                                }
                            }
                        }
                        TaskMessage::IssueComment(ref comment) => {
                            println!("{:?}", comment)
                        }
                        TaskMessage::LogLines(s) => {
                            println!("{s}");
                        }
                    }
                    // match dummy_send.send(msg) {
                    //     Ok(_) => {}
                    //     Err(e) => {
                    //         tracing::error!("dummy_send: {}", e);
                    //     }
                    // }
                }
                // Err(flume::TryRecvError::Empty) => {
                //     tokio::time::sleep(Duration::from_secs(0)).await;
                //     continue;
                // }
                None => {
                    if let Some(registry) = TASK_REGISTRY.get() {
                        let mut registry = registry.entries.write().await;
                        if let Some(entry) = registry.get_mut(&broadcast_registry_key) {
                            // Only overwrite "running" on exit. Leave Error messages alone.
                            if entry.state == TaskState::Running {
                                entry.state = TaskState::Stopped
                            }
                        }
                    }
                    return Ok(());
                }
            }
        }
    });

    // TODO is this necessary??
    let mut reader_broadcast = registry_broadcast.resubscribe();
    let _reader_handle = tokio::spawn(async move {
        loop {
            let _res = reader_broadcast.recv().await;
        }
    });
    let registry = TASK_REGISTRY.get_or_init(TaskRegistry::default);

    let mut registry = registry.entries.write().await;

    if let Some(task) = registry.get(&registry_key)
        && task.state == TaskState::Running
    {
        bail!(
            "Task {} already running for repo: {}/{} at prefix {}",
            name,
            owner,
            repo,
            prefix.to_str().unwrap_or_default()
        )
    }

    // registry_outbox
    //     .send_async(AgentRegistryMessage::ShutDown)
    //     .await?;
    task_outbox.send(TaskMessage::StateChange(TaskState::Running)).await?;

    match &caps["type"] {
        "test" => {
            let mut task = TestTask::new(
                owner,
                repo,
                prefix,
                &caps["path"],
                task_inbox,
                task_outbox.clone(),
                installation_id,
            )
            .await
            .context("TestTask::new()")?;

            let _error_registry_key = registry_key.clone();
            let join_handle = tokio::spawn(async move {
                match task.run(arg).await {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        // tracing::error!("Agent error: {}", e);
                        Ok(task_outbox
                            .send(TaskMessage::StateChange(TaskState::Error {
                                message: format!("{e:#?}"),
                            }))
                            .await?)
                    }
                }
            });

            if blocking {
                join_handle.await??;
            } else {
                registry.insert(
                    registry_key,
                    TaskRegistryEntry {
                        broadcast: registry_broadcast,
                        outbox: registry_outbox,
                        join_handle,
                        state: TaskState::Stopped,
                    },
                );
            }
            Ok(())
        }
        // TODO python reenablement
        // #[cfg(feature = "python")]
        // "op-python" => {
        //     let task = PythonTask::new(
        //         owner,
        //         repo,
        //         prefix,
        //         &caps["path"],
        //         task_inbox,
        //         task_outbox.clone(),
        //         installation_id,
        //     )
        //     .await?;

        //     let error_registry_key = registry_key.clone();
        //     let join_handle = tokio::spawn(async move {
        //         match task.run(arg).await {
        //             Ok(()) => Ok(task_outbox.send(TaskMessage::StateChange(TaskState::Succeeded)).await?),
        //             Err(e) => {
        //                 tracing::error!("Task error: {:#}", e);
        //                 Ok(task_outbox
        //                     .send(TaskMessage::StateChange(TaskState::Error {
        //                         message: format!("{:#?}", e),
        //                     }))
        //                     .await?)
        //             }
        //         }
        //     });
        //     registry.insert(
        //         registry_key,
        //         TaskRegistryEntry {
        //             broadcast: registry_broadcast,
        //             outbox: registry_outbox,
        //             join_handle: join_handle,
        //             state: state::TaskState::Stopped,
        //         },
        //     );
        //     Ok(())
        // }
        _ => Err(AutoschematicError {
            kind: AutoschematicErrorType::InvalidConnectorString(name.to_string()),
        }
        .into()),
    }
}
