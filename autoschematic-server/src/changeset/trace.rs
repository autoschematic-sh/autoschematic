use crate::{
    TRACESTORE,
    error::{AutoschematicServerError, AutoschematicServerErrorType},
    tracestore::TraceHandle,
};

use super::ChangeSet;

pub async fn start_run(
    changeset: &ChangeSet,
    username: &str,
    comment_url: &str,
    r#type: &str,
    command: &str,
) -> anyhow::Result<TraceHandle> {
    let Some(trace_store) = TRACESTORE.get() else {
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "TRACESTORE".to_string(),
                message: "No tracestore configured".to_string(),
            },
        }
        .into());
    };

    trace_store
        .start_run(
            &changeset.owner,
            &changeset.repo,
            changeset.issue_number,
            username,
            comment_url,
            r#type,
            command,
        )
        .await
}

pub async fn append_run_log(handle: &TraceHandle, log: String) -> anyhow::Result<()> {
    let Some(trace_store) = TRACESTORE.get() else {
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "TRACESTORE".to_string(),
                message: "No tracestore configured".to_string(),
            },
        }
        .into());
    };

    trace_store.append_run_log(handle, log).await
}

pub async fn finish_run(handle: &TraceHandle) -> anyhow::Result<()> {
    let Some(trace_store) = TRACESTORE.get() else {
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "TRACESTORE".to_string(),
                message: "No tracestore configured".to_string(),
            },
        }
        .into());
    };

    trace_store.finish_run(handle).await
}
