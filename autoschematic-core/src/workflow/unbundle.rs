use std::{path::Path, sync::Arc};

use anyhow::Context;

use crate::{
    config::AutoschematicConfig,
    connector::{Connector, VirtToPhyOutput},
    connector_cache::ConnectorCache,
    keystore::KeyStore,
    template::template_config,
    report::PlanReport,
    util::split_prefix_addr,
};

pub async fn plan_connector(
    connector_shortname: &str,
    connector: &Box<dyn Connector>,
    prefix: &Path,
    virt_addr: &Path,
) -> Result<Option<PlanReport>, anyhow::Error> {
    let mut plan_report = PlanReport::default();
    plan_report.prefix = prefix.into();
    plan_report.virt_addr = virt_addr.into();

    let phy_addr = match connector.addr_virt_to_phy(virt_addr).await? {
        VirtToPhyOutput::NotPresent => None,
        VirtToPhyOutput::Deferred(read_outputs) => {
            for output in read_outputs {
                plan_report.missing_outputs.push(output);
            }
            return Ok(Some(plan_report));
        }
        VirtToPhyOutput::Present(phy_addr) => Some(phy_addr),
        VirtToPhyOutput::Null(phy_addr) => Some(phy_addr),
    };

    let current = match phy_addr {
        Some(ref phy_addr) => {
            match connector.get(&phy_addr.clone()).await.context(format!(
                "{}::get({})",
                connector_shortname,
                &phy_addr.to_str().unwrap_or_default()
            ))? {
                // Existing resource present for this address
                Some(get_resource_output) => {
                    let resource = get_resource_output.resource_definition;
                    Some(resource)
                }
                // No existing resource present for this address
                None => None,
            }
        }
        None => None,
    };

    let path = prefix.join(virt_addr);

    let connector_ops = if path.is_file() {
        // let desired = std::fs::read(&path)?;
        let desired_bytes = tokio::fs::read(&path).await?;

        match std::str::from_utf8(&desired_bytes) {
            Ok(desired) => {
                let template_result = template_config(prefix, desired)?;

                if !template_result.missing.is_empty() {
                    for read_output in template_result.missing {
                        plan_report.missing_outputs.push(read_output);
                    }

                    return Ok(Some(plan_report));
                } else {
                    // TODO warning that this phy .unwrap_or( virt )
                    // may be the most diabolically awful design
                    // TODO remove awful design
                    connector
                        .plan(
                            &phy_addr.clone().unwrap_or(virt_addr.into()),
                            current,
                            Some(template_result.body.into()),
                        )
                        .await
                        .context(format!("{}::plan({}, _, _)", connector_shortname, virt_addr.display()))?
                }
            }
            Err(_) => {
                // TODO warning that this phy .unwrap_or( virt )
                // may be the most diabolically awful design
                // TODO remove awful design
                connector
                    .plan(&phy_addr.clone().unwrap_or(virt_addr.into()), current, Some(desired_bytes))
                    .await
                    .context(format!("{}::plan({}, _, _)", connector_shortname, virt_addr.display()))?
            }
        }
    } else {
        // The file does not exist, so `desired` is therefore None.
        // Generally speaking, this will destroy the given resource if it currently exists.

        // TODO warning that this phy .unwrap_or( virt )
        // may be the most diabolically awful design
        // TODO remove awful design
        connector
            .plan(&phy_addr.clone().unwrap_or(virt_addr.into()), current, None)
            .await
            .context(format!(
                "{}::plan({}, _, _)",
                connector_shortname,
                virt_addr.to_str().unwrap_or_default()
            ))?
    };

    plan_report.connector_ops = connector_ops;

    Ok(Some(plan_report))
}

/// For a given path, attempt to resolve its prefix and Connector impl and return a Vec of ConnectorOps.
/// Note that this, unlike the server implementation, does not handle setting desired = None where files do
/// not exist - it is intended to be used from the command line or from LSPs to quickly modify resources.
pub async fn unbundle(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: Arc<ConnectorCache>,
    keystore: Option<Arc<dyn KeyStore>>,
    connector_filter: &Option<String>,
    path: &Path,
) -> Result<Option<PlanReport>, anyhow::Error> {
    let autoschematic_config = autoschematic_config.clone();

    let Some((prefix, virt_addr)) = split_prefix_addr(&autoschematic_config, path) else {
        return Ok(None);
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix.to_str().unwrap_or_default()) else {
        return Ok(None);
    };

    let prefix_def = prefix_def.clone();
    let mut handles = Vec::new();
    'connector: for connector_def in prefix_def.connectors {
        if let Some(connector_filter) = &connector_filter
            && connector_def.shortname != *connector_filter {
                continue 'connector;
            }

        let connector_cache = connector_cache.clone();
        let keystore = keystore.clone();
        let prefix = prefix.clone();
        handles.push(tokio::spawn(async move {
            let (connector, mut inbox) = connector_cache
                .get_or_spawn_connector(
                    &connector_def.shortname,
                    &connector_def.spec,
                    &prefix,
                    &connector_def.env,
                    keystore,
                )
                .await
                .unwrap();

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

            connector
        }));

        // if connector_cache.filter(&connector_def.shortname, &prefix, &virt_addr).await? == FilterOutput::Resource {
        //     let plan_report = plan_connector(&connector_def.shortname, &connector, &prefix, &virt_addr).await?;
        //     return Ok(plan_report);
        // }
    }

    Ok(None)
}
