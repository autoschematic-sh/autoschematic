use std::{collections::HashSet, path::PathBuf};

use super::trace::{append_run_log, finish_run, start_run};
use super::util::check_run_url;
use anyhow::Context;
use autoschematic_core::connector::{FilterResponse, OutputMapFile};
use autoschematic_core::{
    connector::{Connector, VirtToPhyResponse},
    glob::addr_matches_filter,
    template::{ReadOutput, template_config},
};
use git2::Repository;
use octocrab::params::checks::{CheckRunConclusion, CheckRunStatus};

use super::ChangeSet;
use crate::{KEYSTORE, object::Object};

#[derive(Default)]
pub struct PullStateReport {
    pub object_count: usize,
    pub import_count: usize,
    pub delete_count: usize,
    pub deferred_count: usize,
    pub missing_outputs: HashSet<ReadOutput>,
}

impl ChangeSet {
    pub async fn pull_state(
        &mut self,
        repo: &Repository,
        subpath: Option<PathBuf>,
        prefix_filter: Option<String>,
        connector_filter: Option<String>,
        comment_username: &str,
        comment_url: &str,
        delete: bool,
    ) -> Result<PullStateReport, anyhow::Error> {
        let mut pull_state_report = PullStateReport::default();

        let trace_handle = start_run(self, comment_username, comment_url, "pull-state", "").await?;

        let autoschematic_config = self.get_autoschematic_config().await?;

        let check_run_url = check_run_url(self, &trace_handle);

        let _chwd = self.chwd_to_repo();
        let subpath = subpath.unwrap_or(PathBuf::from("./"));

        'prefix: for (prefix_name, prefix) in autoschematic_config.prefixes {
            if let Some(prefix_filter) = &prefix_filter
                && prefix_name != *prefix_filter
            {
                continue;
            }

            // let diff_objects = self.get_modified_objects()?;
            let filtered_objects: Vec<&Object> = self
                .objects
                .iter()
                .filter(|object| {
                    let global_addr = &object.filename;
                    if global_addr.starts_with(&prefix_name)
                        && let Ok(virt_addr) = global_addr.strip_prefix(&prefix_name)
                    {
                        // If this address is not under `subpath`, skip it.
                        return addr_matches_filter(virt_addr, &subpath);
                    }
                    false
                })
                .collect();

            if filtered_objects.is_empty() {
                continue 'prefix;
            }

            'connector: for connector_def in prefix.connectors {
                if let Some(connector_filter) = &connector_filter
                    && connector_def.shortname != *connector_filter
                {
                    continue 'connector;
                }

                let (connector, mut inbox) = self
                    .connector_cache
                    .get_or_spawn_connector(
                        &connector_def.shortname,
                        &connector_def.spec,
                        &PathBuf::from(&prefix_name),
                        &connector_def.env,
                        Some(KEYSTORE.clone()),
                        true,
                    )
                    .await?;

                let sender_trace_handle = trace_handle.clone();
                let _reader_handle = tokio::spawn(async move {
                    loop {
                        match inbox.recv().await {
                            Ok(Some(stdout)) => {
                                dbg!(&stdout);
                                let res = append_run_log(&sender_trace_handle, stdout).await;
                                if let Ok(_) = res {}
                            }
                            Ok(None) => {}
                            Err(_) => break,
                        }
                    }
                });

                // let mut connector_import_count = 0;
                'object: for object in &filtered_objects {
                    let Ok(virt_addr) = object.filename.strip_prefix(&prefix_name) else {
                        continue 'object;
                    };
                    tracing::info!("Pull State: {:?}", object.filename.clone());

                    let check_run_name = format!(
                        "autoschematic pull-state -p {} -c {} -s ./{}",
                        prefix_name,
                        &connector_def.shortname,
                        &object.filename.to_string_lossy()
                    );

                    let phy_addr = match connector.addr_virt_to_phy(virt_addr).await? {
                        VirtToPhyResponse::NotPresent => {
                            continue 'object;
                        }
                        VirtToPhyResponse::Deferred(read_outputs) => {
                            pull_state_report.deferred_count += 1;
                            for output in read_outputs {
                                pull_state_report.missing_outputs.insert(output);
                            }
                            continue 'object;
                        }
                        VirtToPhyResponse::Present(phy_addr) => phy_addr,
                        VirtToPhyResponse::Null(phy_addr) => phy_addr,
                    };

                    if self
                        .connector_cache
                        .filter(&connector_def.shortname, &PathBuf::from(&prefix_name), virt_addr)
                        .await?
                        == FilterResponse::Resource
                    {
                        // coz::progress!("pull_state_per_object");
                        let file_check_run_id = self
                            .create_check_run(None, &check_run_name, &check_run_url, CheckRunStatus::InProgress, None)
                            .await?;

                        let desired = if object.filename.is_file() {
                            let desired_bytes = tokio::fs::read(&object.filename).await?;

                            match str::from_utf8(&desired_bytes) {
                                Ok(desired) => {
                                    // If valid utf8, try to template.
                                    let template_result = template_config(&PathBuf::from(&prefix_name), desired)?;

                                    if !template_result.missing.is_empty() {
                                        self.create_check_run(
                                            Some(file_check_run_id),
                                            &check_run_name,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Skipped),
                                        )
                                        .await?;

                                        pull_state_report.deferred_count += 1;
                                        for output in template_result.missing {
                                            pull_state_report.missing_outputs.insert(output);
                                        }

                                        continue 'object;
                                    } else {
                                        template_result.body.into_bytes()
                                    }
                                }
                                Err(_) => desired_bytes,
                            }
                        } else {
                            continue 'object;
                        };

                        let mut tick_import_count = false;
                        let mut tick_delete_count = false;
                        if let Some(current) = connector.get(&phy_addr).await.context(format!(
                            "{}::get({})",
                            connector_def.shortname,
                            &phy_addr.to_str().unwrap_or_default()
                        ))? {
                            if !connector.eq(&phy_addr, &current.resource_definition, &desired).await? {
                                tick_import_count = true;
                                tokio::fs::write(&object.filename, current.resource_definition).await?;
                                self.git_add(repo, &object.filename)?;
                            }

                            if let Some(outputs) = current.outputs
                                && !outputs.is_empty()
                            {
                                tick_import_count = true;

                                let output_map_file = OutputMapFile::OutputMap(outputs);
                                let prefix = PathBuf::from(&prefix_name);

                                let virt_output_path = output_map_file.write(&prefix, virt_addr)?;
                                self.git_add(repo, &virt_output_path)?;

                                if virt_addr != phy_addr {
                                    let phy_output_path = OutputMapFile::write_link(&prefix, &phy_addr, virt_addr)?;
                                    self.git_add(repo, &phy_output_path)?;
                                }
                            }
                        } else if delete {
                            // Resource didn't exist remotely, and `delete` was indicated, so let's delete it!
                            let prefix = PathBuf::from(&prefix_name);

                            if prefix.join(virt_addr).is_file() {
                                tick_delete_count = true;
                                std::fs::remove_file(prefix.join(virt_addr))?;
                                self.git_add(repo, &prefix.join(virt_addr))?;
                            }

                            if let Some(phy_output_path) = OutputMapFile::delete(&prefix, &phy_addr)? {
                                self.git_add(repo, &phy_output_path)?;
                            }

                            if let Some(virt_output_path) = OutputMapFile::delete(&prefix, virt_addr)? {
                                self.git_add(repo, &virt_output_path)?;
                            }
                        };

                        if tick_delete_count {
                            pull_state_report.delete_count += 1;
                        }

                        if tick_import_count {
                            pull_state_report.import_count += 1;
                        }
                    }
                }

                if (pull_state_report.import_count + pull_state_report.delete_count) > 0 {
                    let message = format!(
                        "autoschematic pull-state -c {} by @{}: {}",
                        connector_def.shortname, comment_username, comment_url
                    );
                    self.git_commit_and_push(repo, &message)?;
                }
            }
        }

        finish_run(&trace_handle).await?;
        Ok(pull_state_report)
    }
}
