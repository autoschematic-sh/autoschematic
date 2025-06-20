use anyhow::bail;

use crate::{
    config::AutoschematicConfig,
    connector::{Connector, FilterOutput, VirtToPhyOutput, parse::connector_shortname},
    connector_cache::ConnectorCache,
    connector_util::build_out_path,
    keystore::KeyStore,
    report::{ApplyReport, PlanReport},
    write_output::{link_phy_output_file, unlink_phy_output_file, write_virt_output_file},
};

pub async fn apply_connector(
    connector_shortname: &str,
    connector: &Box<dyn Connector>,
    plan: &PlanReport,
) -> anyhow::Result<Option<ApplyReport>> {
    let mut apply_report = ApplyReport::default();

    for op in &plan.connector_ops {
        // let Some(phy_addr) = connector.addr_virt_to_phy(&virt_addr).await? else {
        //     exec_error = Some(anyhow!(
        //         "Error: virt addr could not be resolved: {:?}",
        //         virt_addr
        //     ));
        //     break;
        // };
        // TODO again, this is the diabolical incongruity between virt_addr and phy_addr depending on
        // the presence of one or the other. Are we really sure this isn't ananas?
        let op_exec_output = match connector.addr_virt_to_phy(&plan.virt_addr).await? {
            VirtToPhyOutput::NotPresent => connector.op_exec(&plan.virt_addr, &op.op_definition).await?,
            VirtToPhyOutput::Deferred(_read_outputs) => {
                bail!("Apply run on plan with deferred outputs.")
            }
            VirtToPhyOutput::Present(phy_addr) => connector.op_exec(&phy_addr, &op.op_definition).await?,
        };

        if let Some(outputs) = &op_exec_output.outputs {
            if !outputs.is_empty() {
                let virt_output_path = build_out_path(&plan.prefix, &plan.virt_addr);

                if let Some(_) = write_virt_output_file(&virt_output_path, outputs, true)? {
                    if let VirtToPhyOutput::Present(phy_addr) = connector.addr_virt_to_phy(&plan.virt_addr).await? {
                        let phy_output_path = build_out_path(&plan.prefix, &phy_addr);

                        if phy_addr != plan.virt_addr {
                            // apply_report.phy_addr = Some(phy_addr.clone());

                            let _phy_output_path = link_phy_output_file(&virt_output_path, &phy_output_path)?;
                            apply_report.wrote_files.push(phy_output_path);
                        }

                        apply_report.wrote_files.push(virt_output_path);
                    }
                } else if let VirtToPhyOutput::Present(phy_addr) = connector.addr_virt_to_phy(&plan.virt_addr).await? {
                    let phy_output_path = build_out_path(&plan.prefix, &phy_addr);

                    if phy_addr != plan.virt_addr {
                        unlink_phy_output_file(&phy_output_path)?;
                        apply_report.wrote_files.push(phy_output_path);
                    }

                    apply_report.wrote_files.push(virt_output_path);
                }
            }
        }

        apply_report.outputs.push(op_exec_output);
    }

    Ok(Some(apply_report))
}

/// For a given path, attempt to resolve its prefix and Connector impl and return a Vec of ConnectorOps.
/// Note that this, unlike the server implementation, does not handle setting desired = None where files do
/// not exist - it is intended to be used from the command line or from LSPs to quickly modify resources.
pub async fn apply(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<&Box<dyn KeyStore>>,
    connector_filter: &Option<String>,
    plan_report: &PlanReport,
) -> Result<Option<ApplyReport>, anyhow::Error> {
    // let Some((prefix, virt_addr)) = split_prefix_addr(autoschematic_config, path) else {
    //     return Ok(None);
    // };

    let Some(prefix_def) = autoschematic_config
        .prefixes
        .get(plan_report.prefix.to_str().unwrap_or_default())
    else {
        return Ok(None);
    };

    'connector: for connector_def in &prefix_def.connectors {
        let connector_shortname = connector_shortname(&connector_def.name)?;

        if let Some(connector_filter) = &connector_filter {
            if connector_shortname != *connector_filter {
                continue 'connector;
            }
        }

        let (connector, mut inbox) = connector_cache
            .get_or_spawn_connector(&connector_def.name, &plan_report.prefix, &connector_def.env, keystore)
            .await?;

        let _reader_handle = tokio::spawn(async move {
            loop {
                match inbox.recv().await {
                    Ok(Some(stdout)) => {
                        // let res = append_run_log(&sender_trace_handle, stdout).await;
                        eprintln!("{}", stdout);
                        // match res {
                        //     Ok(_) => {}
                        //     Err(_) => {}
                        // }
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });

        if connector_cache
            .filter(&connector_def.name, &plan_report.prefix, &plan_report.virt_addr)
            .await?
            == FilterOutput::Resource
        {
            let apply_report = apply_connector(&connector_shortname, &connector, plan_report).await?;
            return Ok(apply_report);
        }
    }

    Ok(None)
}
