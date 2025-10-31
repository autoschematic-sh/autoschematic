use std::path::PathBuf;

use super::trace::{append_run_log, finish_run, start_run};
use anyhow::bail;
use autoschematic_core::config_rbac;
use autoschematic_core::config_rbac::AutoschematicRbacConfig;
use autoschematic_core::report::ApplyReport;
use autoschematic_core::report::ApplyReportSet;
use autoschematic_core::workflow;
use git2::Repository;
use octocrab::params::checks::{CheckRunConclusion, CheckRunStatus};

use super::ChangeSet;
use crate::{DOMAIN, KEYSTORE};

impl ChangeSet {
    #[allow(clippy::too_many_arguments)]
    pub async fn apply(
        &mut self,
        repo: &Repository,
        _subpath: Option<PathBuf>,
        connector_filter: Option<String>,
        comment_username: &str,
        comment_url: &str,
        rbac_config: &AutoschematicRbacConfig,
        rbac_user: &config_rbac::User,
    ) -> Result<ApplyReportSet, anyhow::Error> {
        let trace_handle = start_run(self, comment_username, comment_url, "apply", "").await?;
        let mut pr_approvals = None;

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

        let autoschematic_config = self.get_autoschematic_config().await?;

        let _chwd = self.chwd_to_repo();
        for plan_report in &plan_report_set.plan_reports {
            let Some(ref connector_def) = plan_report.connector_def else {
                continue;
            };

            if let Some(connector_filter) = &connector_filter
                && connector_def.shortname != *connector_filter
            {
                continue;
            }

            if !rbac_config.allows_apply_without_approval(
                rbac_user,
                plan_report.prefix.to_string_lossy().as_ref(),
                &connector_def.shortname,
            ) {
                if !rbac_config.allows_apply_with_approval(
                    rbac_user,
                    plan_report.prefix.to_string_lossy().as_ref(),
                    &connector_def.shortname,
                ) {
                    continue;
                }

                if pr_approvals.is_none() {
                    pr_approvals = Some(self.get_pr_approvals().await?);
                }

                if let Some(ref pr_approvals) = pr_approvals {
                    if !rbac_config.allows_apply_if_approved_by(
                        rbac_user,
                        plan_report.prefix.to_string_lossy().as_ref(),
                        &connector_def.shortname,
                        pr_approvals,
                    ) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            let virt_addr = plan_report.virt_addr.clone();
            // let phy_addr = plan_report.phy_addr.clone();
            let prefix = plan_report.prefix.clone();
            let Some(prefix_name) = prefix.to_str() else {
                continue;
            };

            let check_run_name = format!(
                "autoschematic apply -c {} -p ./{:?}",
                &connector_def.shortname,
                &PathBuf::from(&prefix).join(&virt_addr)
            );

            let Some(ref connector_def) = plan_report.connector_def else {
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

            if let Some(ref _error) = plan_report.error {
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
                    &autoschematic_config,
                    &prefix_name,
                    &connector_def,
                    Some(KEYSTORE.clone()),
                    true,
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

                match workflow::apply::apply_connector(connector, plan_report).await {
                    Ok(Some(apply_report)) => apply_report_set.apply_reports.push(apply_report),
                    Ok(None) => continue,
                    Err(e) => exec_error = Some(e),
                }

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
                            connector_shortname: connector_def.shortname.clone(),
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
                            connector_shortname: connector_def.shortname.clone(),
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
