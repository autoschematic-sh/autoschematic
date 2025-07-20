use std::path::PathBuf;

use super::trace::{append_run_log, finish_run, start_run};
use anyhow::bail;
use autoschematic_core::report::ApplyReport;
use autoschematic_core::report::ApplyReportSet;
use autoschematic_core::workflow;
use git2::Repository;
use octocrab::params::checks::{CheckRunConclusion, CheckRunStatus};

use super::ChangeSet;
use crate::{DOMAIN, KEYSTORE};

impl ChangeSet {
    pub async fn apply(
        &mut self,
        repo: &Repository,
        _subpath: Option<PathBuf>,
        connector_filter: Option<String>,
        comment_username: &str,
        comment_url: &str,
    ) -> Result<ApplyReportSet, anyhow::Error> {
        let trace_handle = start_run(self, comment_username, comment_url, "apply", "").await?;

        let mut apply_report_set = ApplyReportSet::default();

        let check_run_url = match DOMAIN.get() {
            Some(domain) => format!("https://{domain}"),
            None => String::new(),
        };

        let Some(plan_report_set) = &self.last_plan else {
            bail!("No saved plan found!")
        };

        if plan_report_set.apply_success {
            bail!("Stored plan already fully executed!")
        };

        let _chwd = self.chwd_to_repo();
        for plan_report in &plan_report_set.plan_reports {
            if let Some(connector_filter) = &connector_filter
                && plan_report.connector_shortname != *connector_filter {
                    continue;
                }

            let virt_addr = plan_report.virt_addr.clone();
            // let phy_addr = plan_report.phy_addr.clone();
            let prefix = plan_report.prefix.clone();

            let check_run_name = format!(
                "autoschematic apply -c {} -p ./{:?}",
                &plan_report.connector_shortname,
                &PathBuf::from(&prefix).join(&virt_addr)
            );

            let Some(ref connector_spec) = plan_report.connector_spec else {
                let _file_check_run_id = self
                    .create_check_run(
                        None,
                        &check_run_name,
                        &check_run_url,
                        CheckRunStatus::Completed,
                        Some(CheckRunConclusion::Skipped),
                    )
                    .await?;

                continue;
            };

            if let Some(ref error) = plan_report.error {
                let _file_check_run_id = self
                    .create_check_run(
                        None,
                        &check_run_name,
                        &check_run_url,
                        CheckRunStatus::Completed,
                        Some(CheckRunConclusion::Skipped),
                    )
                    .await?;

                continue;
            }

            let (connector, mut inbox) = self
                .connector_cache
                .get_or_spawn_connector(
                    &plan_report.connector_shortname,
                    connector_spec,
                    &PathBuf::from(&prefix),
                    &plan_report.connector_env,
                    Some(KEYSTORE.clone()),
                )
                .await?;
            let sender_trace_handle = trace_handle.clone();
            let _reader_handle = tokio::spawn(async move {
                loop {
                    match inbox.recv().await {
                        Ok(Some(stdout)) => {
                            let _res = append_run_log(&sender_trace_handle, stdout).await;
                            // match res {
                            //     Ok(r) => {}
                            //     Err(e) => {}
                            // }
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            });

            if !plan_report.connector_ops.is_empty() {
                let file_check_run_id = self
                    .create_check_run(None, &check_run_name, &check_run_url, CheckRunStatus::InProgress, None)
                    .await?;

                let op_exec_outputs = Vec::new();
                let wrote_files = Vec::new();
                let mut exec_error = None;
                let report_phy_addr: Option<PathBuf> = None;

                match workflow::apply::apply_connector(&plan_report.connector_shortname, connector, plan_report).await {
                    Ok(Some(apply_report)) => apply_report_set.apply_reports.push(apply_report),
                    Ok(None) => continue,
                    Err(e) => exec_error = Some(e),
                }

                /*                for op in &plan_report.connector_ops {
                    // let Some(phy_addr) = connector.addr_virt_to_phy(&virt_addr).await? else {
                    //     exec_error = Some(anyhow!(
                    //         "Error: virt addr could not be resolved: {:?}",
                    //         virt_addr
                    //     ));
                    //     break;
                    // };
                    // TODO again, this is the diabolical incongruity between virt_addr and phy_addr depending on
                    // the presence of one or the other. Are we really sure this isn't bananas?
                    let res = match connector.addr_virt_to_phy(&virt_addr).await? {
                        VirtToPhyOutput::NotPresent => connector.op_exec(&virt_addr, &op.op_definition).await,
                        VirtToPhyOutput::Deferred(_read_outputs) => connector.op_exec(&virt_addr, &op.op_definition).await,
                        VirtToPhyOutput::Present(phy_addr) => connector.op_exec(&phy_addr, &op.op_definition).await,
                        VirtToPhyOutput::Null(phy_addr) => connector.op_exec(&phy_addr, &op.op_definition).await,
                    };

                    match res {
                        Ok(op_exec_output) => {
                            if let Some(outputs) = &op_exec_output.outputs {
                                if !outputs.is_empty() {
                                    let virt_output_path = build_out_path(&PathBuf::from(&prefix), &virt_addr);

                                    if let Some(_) = write_virt_output_file(&virt_output_path, outputs, true)? {
                                        if let VirtToPhyOutput::Present(phy_addr) =
                                            connector.addr_virt_to_phy(&virt_addr).await?
                                        {
                                            let phy_output_path = build_out_path(&PathBuf::from(&prefix), &phy_addr);

                                            if phy_addr != virt_addr {
                                                report_phy_addr = Some(phy_addr.clone());

                                                let _phy_output_path =
                                                    link_phy_output_file(&virt_output_path, &phy_output_path)?;
                                                wrote_files.push(phy_output_path);
                                            }

                                            wrote_files.push(virt_output_path);
                                        }
                                    } else if let VirtToPhyOutput::Present(phy_addr) =
                                        connector.addr_virt_to_phy(&virt_addr).await?
                                    {
                                        let phy_output_path = build_out_path(&PathBuf::from(&prefix), &phy_addr);

                                        if phy_addr != virt_addr {
                                            unlink_phy_output_file(&phy_output_path)?;
                                            wrote_files.push(phy_output_path);
                                        }

                                        wrote_files.push(virt_output_path);
                                    }
                                }
                            }

                            op_exec_outputs.push(op_exec_output);
                        }
                        Err(e) => {
                            exec_error = e.into();
                            // TODO add continue_on_error option? I doubt it...
                            break;
                        }
                    }
                } */

                match exec_error {
                    Some(e) => {
                        apply_report_set.overall_success = false;
                        self.create_check_run(
                            Some(file_check_run_id),
                            &check_run_name,
                            &check_run_url,
                            CheckRunStatus::Completed,
                            Some(CheckRunConclusion::Failure),
                        )
                        .await?;
                        apply_report_set.apply_reports.push(ApplyReport {
                            connector_shortname: plan_report.connector_shortname.clone(),
                            prefix: prefix.clone(),
                            virt_addr: virt_addr.to_path_buf(),
                            phy_addr: Some(PathBuf::new()),
                            wrote_files: Vec::new(),
                            outputs: op_exec_outputs,
                            error: Some(e.into()),
                        });
                    }
                    None => {
                        self.create_check_run(
                            Some(file_check_run_id),
                            &check_run_name,
                            &check_run_url,
                            CheckRunStatus::Completed,
                            Some(CheckRunConclusion::Success),
                        )
                        .await?;
                        apply_report_set.apply_reports.push(ApplyReport {
                            connector_shortname: plan_report.connector_shortname.clone(),
                            prefix: prefix.clone(),
                            virt_addr,
                            phy_addr: report_phy_addr,
                            wrote_files,
                            outputs: op_exec_outputs,
                            error: None,
                        });
                    }
                }
            } else {
                let _file_check_run_id = self
                    .create_check_run(
                        None,
                        &check_run_name,
                        &check_run_url,
                        CheckRunStatus::Completed,
                        Some(CheckRunConclusion::Skipped),
                    )
                    .await?;
            }
        }

        for apply_report in &apply_report_set.apply_reports {
            let mut count = 0;
            for file in &apply_report.wrote_files {
                count += 1;
                self.git_add(repo, file)?;
            }
            // for output in &apply_report.outputs {
            //     if let Some(outputs) = &output.outputs {
            //         if outputs.len() > 0 {
            //             count += 1;

            //             let virt_output_path = build_out_path(
            //                 &PathBuf::from(apply_report.prefix.clone()),
            //                 &apply_report.virt_addr,
            //             );
            //             self.git_add(repo, &virt_output_path)?;

            //             if let Some(phy_addr) = apply_report.phy_addr.clone() {
            //                 if phy_addr != apply_report.virt_addr {
            //                     let phy_output_path = build_out_path(
            //                         &PathBuf::from(apply_report.prefix.clone()),
            //                         &phy_addr,
            //                     );
            //                     self.git_add(repo, &phy_output_path)?;
            //                 }
            //             }
            //         }
            //     }
            // }
            if count > 0 {
                let message = format!("autoschematic apply by @{comment_username}: {comment_url}");
                self.git_commit_and_push(repo, &message)?;
            }
        }

        if let Some(plan_report_set) = self.last_plan.as_mut() {
            plan_report_set.apply_success = true;
        };

        finish_run(&trace_handle).await?;
        Ok(apply_report_set)
    }
}
