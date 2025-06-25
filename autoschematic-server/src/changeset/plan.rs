use std::{
    collections::HashSet,
    fs::{self},
    path::PathBuf,
};

use super::trace::{append_run_log, finish_run, start_run};
use super::util::check_run_url;
use anyhow::Context;
use autoschematic_core::report::{PlanReport, PlanReportSet, PlanReportSetOld};
use autoschematic_core::{connector::FilterOutput, report::PlanReportOld};
use autoschematic_core::{
    connector::{Connector, VirtToPhyOutput, parse::connector_shortname},
    glob::addr_matches_filter,
    read_outputs::template_config,
};
use git2::Repository;
use octocrab::params::checks::{CheckRunConclusion, CheckRunStatus};

use super::ChangeSet;
use crate::{KEYSTORE, object::Object};

impl ChangeSet {
    pub async fn plan(
        &mut self,
        _repo: &Repository,
        subpath: Option<PathBuf>,
        prefix_filter: &Option<String>,
        connector_filter: &Option<String>,
        comment_username: &str,
        comment_url: &str,
        continue_on_error: bool,
    ) -> Result<(), anyhow::Error> {
        let trace_handle = start_run(self, comment_username, comment_url, "plan", "").await?;

        let autoschematic_config = self.autoschematic_config().await?;

        let check_run_url = check_run_url(self, &trace_handle);

        let _chwd = self.chwd_to_repo();
        let subpath = subpath.unwrap_or(PathBuf::from("./"));

        let mut plan_report_set = PlanReportSet {
            overall_success: true,
            apply_success: false,
            plan_reports: Vec::new(),
            object_count: 0,
            deferred_count: 0,
            deferred_pending_outputs: HashSet::new(),
        };

        'prefix: for (prefix_name, prefix) in autoschematic_config.prefixes {
            if let Some(prefix_filter) = &prefix_filter {
                if prefix_name != *prefix_filter {
                    continue;
                }
            }

            // let diff_objects = self.get_modified_objects()?;
            let filtered_objects: Vec<&Object> = self
                .objects
                .iter()
                .filter(|object| {
                    let global_addr = &object.filename;
                    if global_addr.starts_with(&prefix_name) {
                        if let Ok(virt_addr) = global_addr.strip_prefix(&prefix_name) {
                            // If this address is not under `subpath`, skip it.
                            return addr_matches_filter(&PathBuf::from(&prefix_name), virt_addr, &subpath);
                        }
                    }
                    false
                })
                .collect();

            if filtered_objects.is_empty() {
                continue 'prefix;
            }

            'connector: for connector_def in prefix.connectors {
                if let Some(connector_filter) = &connector_filter {
                    if connector_def.shortname != *connector_filter {
                        continue 'connector;
                    }
                }

                let (connector, mut inbox) = self
                    .connector_cache
                    .get_or_spawn_connector(
                        &connector_def.shortname,
                        &connector_def.spec,
                        &PathBuf::from(&prefix_name),
                        &connector_def.env,
                        Some(&KEYSTORE),
                    )
                    .await?;
                let sender_trace_handle = trace_handle.clone();
                let _reader_handle = tokio::spawn(async move {
                    loop {
                        match inbox.recv().await {
                            Ok(Some(stdout)) => {
                                let res = append_run_log(&sender_trace_handle, stdout).await;
                                if let Ok(_) = res {}
                            }
                            Ok(None) => {}
                            Err(_) => break,
                        }
                    }
                });

                'object: for object in &filtered_objects {
                    let Ok(virt_addr) = object.filename.strip_prefix(&prefix_name) else {
                        continue;
                    };

                    tracing::info!("Plan: {:?}", object.filename.clone());

                    let check_run_name = format!(
                        "autoschematic plan -c {} -p ./{:?}",
                        &connector_def.shortname, &object.filename
                    );

                    let phy_addr = match connector.addr_virt_to_phy(virt_addr).await? {
                        VirtToPhyOutput::NotPresent => None,
                        VirtToPhyOutput::Deferred(read_outputs) => {
                            plan_report_set.deferred_count += 1;
                            for output in read_outputs {
                                plan_report_set.deferred_pending_outputs.insert(output);
                            }
                            continue 'object;
                        }
                        VirtToPhyOutput::Present(phy_addr) => Some(phy_addr),
                        VirtToPhyOutput::Null(phy_addr) => Some(phy_addr),
                    };

                    if self
                        .connector_cache
                        .filter(&connector_def.shortname, &PathBuf::from(&prefix_name), virt_addr)
                        .await?
                        == FilterOutput::Resource
                    {
                        // coz::progress!("plan_per_object");
                        plan_report_set.object_count += 1;

                        let file_check_run_id = self
                            .create_check_run(None, &check_run_name, &check_run_url, CheckRunStatus::InProgress, None)
                            .await?;

                        let current = match phy_addr {
                            Some(ref phy_addr) => {
                                match connector.get(&phy_addr.clone()).await.context(format!(
                                    "{}::get({})",
                                    connector_def.shortname,
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

                        let reads_outputs = Vec::new();
                        let plan = if object.filename.is_file() {
                            let desired = fs::read_to_string(&object.filename)?;

                            // reads_outputs.append(&mut get_read_outputs(&desired));

                            let template_result = template_config(&PathBuf::from(&prefix_name), &desired)?;

                            if !template_result.missing.is_empty() {
                                self.create_check_run(
                                    Some(file_check_run_id),
                                    &check_run_name,
                                    &check_run_url,
                                    CheckRunStatus::Completed,
                                    Some(CheckRunConclusion::Skipped),
                                )
                                .await?;

                                for read_output in template_result.missing {
                                    plan_report_set.deferred_pending_outputs.insert(read_output);
                                }

                                plan_report_set.deferred_count += 1;
                                continue 'object;
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
                                    .context(format!(
                                        "{}::plan({}, _, _)",
                                        connector_def.shortname,
                                        virt_addr.to_str().unwrap_or_default()
                                    ))
                            }
                        } else {
                            // TODO warning that this phy .unwrap_or( virt )
                            // may be the most diabolically awful design
                            // TODO remove awful design
                            connector
                                .plan(&phy_addr.clone().unwrap_or(virt_addr.into()), current, None)
                                .await
                                .context(format!(
                                    "{}::plan({}, _, _)",
                                    connector_def.shortname,
                                    virt_addr.to_str().unwrap_or_default()
                                ))
                        };

                        match plan {
                            Ok(connector_ops) => {
                                self.create_check_run(
                                    Some(file_check_run_id),
                                    &check_run_name,
                                    &check_run_url,
                                    CheckRunStatus::Completed,
                                    Some(CheckRunConclusion::Success),
                                )
                                .await?;

                                plan_report_set.plan_reports.push(PlanReport {
                                    connector_shortname: connector_def.shortname.clone(),
                                    connector_spec: Some(connector_def.spec.clone()),
                                    connector_env: connector_def.env.clone(),
                                    prefix: PathBuf::from(&prefix_name),
                                    virt_addr: virt_addr.to_path_buf(),
                                    phy_addr: phy_addr.clone(),
                                    connector_ops,
                                    reads_outputs,
                                    error: None,
                                    missing_outputs: Vec::new(),
                                });
                            }

                            Err(e) => {
                                self.create_check_run(
                                    Some(file_check_run_id),
                                    &check_run_name,
                                    &check_run_url,
                                    CheckRunStatus::Completed,
                                    Some(CheckRunConclusion::Failure),
                                )
                                .await?;

                                plan_report_set.overall_success = false;
                                plan_report_set.plan_reports.push(PlanReport {
                                    connector_shortname: connector_def.shortname.clone(),
                                    connector_spec: Some(connector_def.spec.clone()),
                                    connector_env: connector_def.env.clone(),
                                    prefix: PathBuf::from(&prefix_name),
                                    virt_addr: virt_addr.to_path_buf(),
                                    phy_addr,
                                    connector_ops: Vec::new(),
                                    reads_outputs: Vec::new(),
                                    missing_outputs: Vec::new(),
                                    error: Some(e.into()),
                                });
                                if !continue_on_error {
                                    break 'prefix;
                                }
                            }
                        }
                    }
                }
            }
        }

        finish_run(&trace_handle).await?;

        // // TODO actually send a message...
        // // Have deferrals? Let's make sure we're not in a loop!
        // if !plan_report_set.deferred_pending_outputs.is_empty() {
        //     if let Some(last_plan) = &self.last_plan {
        //         // If we fully applied the last plan, and we're still deferring on the
        //         // same output keys, we're in a loop!
        //         if last_plan.apply_success && (last_plan.deferred_pending_outputs == plan_report_set.deferred_pending_outputs) {
        //             // bail!("Loop detected! The last plan ran fully, but still deferred on keys")
        //         }
        //     }
        // }

        self.last_plan = Some(plan_report_set);
        Ok(())
    }
}
