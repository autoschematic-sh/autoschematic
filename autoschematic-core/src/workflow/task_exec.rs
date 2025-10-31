use std::{path::Path, sync::Arc};

use anyhow::Context;
use tokio::task::JoinSet;

use crate::{
    config::AutoschematicConfig,
    connector::{Connector, FilterResponse, TaskExecResponse, VirtToPhyResponse},
    connector_cache::ConnectorCache,
    keystore::KeyStore,
    template::template_config,
    util::split_prefix_addr,
};

pub async fn task_exec_connector(
    connector_shortname: &str,
    connector: Arc<dyn Connector>,
    prefix: &Path,
    virt_addr: &Path,
    arg: Option<Arc<Vec<u8>>>,
    state: Option<Arc<Vec<u8>>>,
) -> Result<Option<TaskExecResponse>, anyhow::Error> {
    let _phy_addr = match connector.addr_virt_to_phy(virt_addr).await? {
        VirtToPhyResponse::NotPresent => None,
        VirtToPhyResponse::Deferred(_read_outputs) => {
            // TODO again, how do we encode missing outputs in TaskExecResponse? Do we?
            return Ok(None);
        }
        VirtToPhyResponse::Present(phy_addr) => Some(phy_addr),
        VirtToPhyResponse::Null(phy_addr) => Some(phy_addr),
    };

    let path = prefix.join(virt_addr);

    if !path.is_file() {
        return Ok(None);
    }

    let task_body_bytes = tokio::fs::read(&path).await?;

    match std::str::from_utf8(&task_body_bytes) {
        Ok(desired) => {
            let template_result = template_config(prefix, desired)?;

            if !template_result.missing.is_empty() {
                for _read_output in template_result.missing {
                    // TODO: Will we template task bodies? (Sure, right?)
                    // Then, how will we encode missing outputs in TaskExecResponse?
                    // plan_report.missing_outputs.push(read_output);
                    // Right now, we silently fail and return None!
                }

                Ok(None)
            } else {
                // TODO c'mon, surely we can avoid this cloned() call here...
                let task_exec_resp = connector
                    .task_exec(
                        virt_addr,
                        template_result.body.into_bytes(),
                        arg.as_deref().cloned(),
                        state.as_deref().cloned(),
                    )
                    .await
                    .context(format!("{}::task_exec({}, _, _)", connector_shortname, virt_addr.display()))?;
                Ok(Some(task_exec_resp))
            }
        }
        Err(_) => {
            let task_exec_resp = connector
                .task_exec(virt_addr, task_body_bytes, arg.as_deref().cloned(), state.as_deref().cloned())
                .await
                .context(format!("{}::task_exec({}, _, _)", connector_shortname, virt_addr.display()))?;
            Ok(Some(task_exec_resp))
        }
    }
}

/// For a given path, attempt to resolve its prefix and Connector impl and carry out one iteration of task_exec.
pub async fn task_exec(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: Arc<ConnectorCache>,
    keystore: Option<Arc<dyn KeyStore>>,
    connector_filter: &Option<String>,
    path: &Path,
    arg: Option<Vec<u8>>,
    state: Option<Vec<u8>>,
) -> Result<Option<TaskExecResponse>, anyhow::Error> {
    let autoschematic_config = Arc::new(autoschematic_config.clone());

    let Some((prefix, virt_addr)) = split_prefix_addr(&autoschematic_config, path) else {
        // eprintln!("split_prefix_addr None!");
        return Ok(None);
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix.to_str().unwrap_or_default()) else {
        // eprintln!("prefix None!");
        return Ok(None);
    };

    let autoschematic_config = autoschematic_config.clone();
    let prefix_def = prefix_def.clone();
    let arg = arg.map(Arc::new);
    let state = state.map(Arc::new);

    let mut joinset: JoinSet<anyhow::Result<Option<TaskExecResponse>>> = JoinSet::new();

    'connector: for connector_def in prefix_def.connectors {
        if let Some(connector_filter) = &connector_filter
            && connector_def.shortname != *connector_filter
        {
            continue 'connector;
        }

        let autoschematic_config = autoschematic_config.clone();
        let connector_cache = connector_cache.clone();
        let keystore = keystore.clone();
        let prefix = prefix.clone();
        let virt_addr = virt_addr.clone();

        let arg = arg.clone();
        let state = state.clone();
        joinset.spawn(async move {
            let Some(prefix_name) = prefix.to_str() else {
                return Ok(None);
            };

            let (connector, mut inbox) = connector_cache
                .get_or_spawn_connector(&autoschematic_config, &prefix_name, &connector_def, keystore, true)
                .await?;

            let _reader_handle = tokio::spawn(async move {
                loop {
                    match inbox.recv().await {
                        Ok(Some(stdout)) => {
                            eprintln!("{stdout}");
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            });

            if connector_cache
                .filter_cached(&connector_def.shortname, &prefix, &virt_addr)
                .await?
                == FilterResponse::Resource
            {
                let task_exec_resp =
                    task_exec_connector(&connector_def.shortname, connector, &prefix, &virt_addr, arg, state).await?;
                return Ok(task_exec_resp);
            }
            Ok(None)
            // return connector;
        });

        // if connector_cache.filter(&connector_def.shortname, &prefix, &virt_addr).await? == FilterResponse::Resource {
        //     let plan_report = plan_connector(&connector_def.shortname, &connector, &prefix, &virt_addr).await?;
        //     return Ok(plan_report);
        // }
    }

    while let Some(res) = joinset.join_next().await {
        if let Some(plan_report) = res?? {
            return Ok(Some(plan_report));
        }
    }

    Ok(None)
}
