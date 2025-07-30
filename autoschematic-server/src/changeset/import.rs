use std::{
    fs::{self},
    path::{Path, PathBuf},
    sync::Arc,
};

use super::trace::{append_run_log, finish_run, start_run};
use anyhow::Context;
use autoschematic_core::{
    config_rbac::{self, AutoschematicRbacConfig},
    connector::{Connector, OutputMapFile},
    glob::addr_matches_filter,
};
use git2::{Cred, IndexAddOption, PushOptions, RemoteCallbacks, Repository};
use secrecy::ExposeSecret;
use tokio::sync::broadcast::error::RecvError;

use crate::{KEYSTORE, error::AutoschematicServerError};

use super::ChangeSet;

impl ChangeSet {
    pub async fn import_resource(
        &self,
        repo: &Repository,
        connector_shortname: &str,
        connector: Arc<dyn Connector>,
        prefix: &Path,
        phy_addr: &Path,
        overwrite_existing: bool,
    ) -> Result<bool, anyhow::Error> {
        let phy_addr = if phy_addr.is_absolute() {
            phy_addr.strip_prefix("/")?
        } else {
            phy_addr
        };
        let path = PathBuf::from(prefix).join(phy_addr);

        // for some/prefix/connector/type/resource.txt,
        // make sure some/prefix/connector/type exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let phy_out_path = OutputMapFile::path(prefix, phy_addr);

        if path.exists() && !overwrite_existing {
            // Here, the physical address returned by list() already
            // has a corresponding file in the repo.
            // tracing::info!("import: already exists at path: {:?}", path);
        } else if phy_out_path.exists() && !overwrite_existing {
            // Here, the output file corresponding to the physical address returned by list() already
            // exists. This may be the real output file, or a symlink to the output file
            // corresponding to a virtual address.
            // tracing::info!("import: already exists at path: {:?}", path);
        } else {
            tracing::info!("import at path: {:?}", path);

            let virt_addr = connector.addr_phy_to_virt(phy_addr).await?.unwrap_or(phy_addr.to_path_buf());

            tracing::error!("addr_phy_to_virt: {:?} -> {:?}", phy_addr, virt_addr);

            match connector
                .get(phy_addr)
                .await
                .context(format!("{connector_shortname}::get()"))?
            {
                // Dump the found resource to a string and commit and push!
                Some(get_resource_output) => {
                    let body = get_resource_output.resource_definition;
                    let res_path = prefix.join(&virt_addr);

                    if let Some(parent) = res_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    tokio::fs::write(&res_path, body).await?;
                    tracing::info!("import completed...");

                    let mut index = repo.index()?;

                    index.add_all([res_path], IndexAddOption::default(), None)?;
                    index.write()?;

                    if let Some(outputs) = get_resource_output.outputs
                        && !outputs.is_empty()
                    {
                        let output_map_file = OutputMapFile::OutputMap(outputs);

                        let virt_output_path = output_map_file.write(prefix, &virt_addr)?;
                        self.git_add(repo, &virt_output_path)?;

                        // TODO can import ever delete/unlink an output file?
                        if virt_addr != phy_addr {
                            let phy_output_path = OutputMapFile::write_link(prefix, phy_addr, &virt_addr)?;
                            self.git_add(repo, &phy_output_path)?;
                        }
                    }

                    return Ok(true);
                }
                None => {
                    tracing::error!("No remote resource at addr:{:?} path: {:?}", phy_addr, path);
                    // TODO bail on an error here, this indicates a probable connector bug!
                }
            }
        }
        Ok(false)
    }

    pub async fn import_all(
        &self,
        subpath: Option<PathBuf>,
        prefix_filter: Option<String>,
        connector_filter: Option<String>,
        comment_username: &str,
        comment_url: &str,
        overwrite_existing: bool,
        rbac_config: &AutoschematicRbacConfig,
        rbac_user: &config_rbac::User,
    ) -> Result<(usize, usize), AutoschematicServerError> {
        let trace_handle = start_run(self, comment_username, comment_url, "import", "").await?;

        let repo = self.clone_repo().await?;

        let autoschematic_config = self.get_autoschematic_config().await?;

        let resource_group_map = autoschematic_config.resource_group_map();

        let subpath = subpath.unwrap_or(PathBuf::from("./"));

        // Number of resources found and imported
        let mut imported_count: usize = 0;
        // Number of resources found
        let mut total_count: usize = 0;

        for (prefix_name, prefix) in autoschematic_config.prefixes {
            if let Some(prefix_filter) = &prefix_filter
                && prefix_name != *prefix_filter
            {
                continue;
            }
            for connector_def in prefix.connectors {
                let connector_shortname = &connector_def.shortname;
                if let Some(connector_filter) = &connector_filter
                    && connector_shortname != connector_filter
                {
                    continue;
                }

                if !rbac_config.allows_read(rbac_user, &prefix_name, &connector_shortname) {
                    tracing::info!(
                        "RBAC denied for user {:?} in prefix {:?} with connector {}",
                        rbac_user,
                        prefix_name,
                        connector_shortname
                    );
                    continue;
                }

                let prefix_name = PathBuf::from(&prefix_name);

                // subcount represents the number of resources imported by this connector,
                // count represents the number of resources imported by all connectors
                let mut imported_subcount: usize = 0;
                // temporarily cd into the repo...
                // I.E. `cd /tmp/autoschematic-298347928/pfnsec/autoschematic-playground`
                let _chwd = self.chwd_to_repo();

                let (connector, mut inbox) = self
                    .connector_cache
                    .get_or_spawn_connector(
                        &connector_def.shortname,
                        &connector_def.spec,
                        &PathBuf::from(&prefix_name),
                        &connector_def.env,
                        Some(KEYSTORE.clone()),
                        true
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
                            Err(RecvError::Closed) => break,
                            _ => {}
                        }
                    }
                });

                let phy_addrs = connector.list(&subpath.clone()).await.context(format!(
                    "{}::list({})",
                    connector_shortname,
                    subpath.to_str().unwrap_or_default()
                ))?;

                'phy_addr: for phy_addr in phy_addrs {
                    if !addr_matches_filter(&phy_addr, &subpath) {
                        continue 'phy_addr;
                    }

                    // Skip files that already exist in other resource groups.
                    if let Some(ref resource_group) = prefix.resource_group
                        && let Some(neighbour_prefixes) = resource_group_map.get(resource_group)
                    {
                        // get all prefixes in this resource group except our own
                        for neighbour_prefix in neighbour_prefixes.iter().filter(|p| **p != prefix_name) {
                            if neighbour_prefix.join(&phy_addr).exists() {
                                continue 'phy_addr;
                            }

                            if OutputMapFile::path(neighbour_prefix, &phy_addr).exists() {
                                continue 'phy_addr;
                            }
                        }
                    }

                    let res = self
                        .import_resource(
                            &repo,
                            &connector_shortname,
                            connector.clone(),
                            &prefix_name,
                            &phy_addr,
                            overwrite_existing,
                        )
                        .await?;
                    if res {
                        imported_subcount += 1;
                        imported_count += 1;
                    }
                    total_count += 1;
                }

                // TODO sign commits with a private key
                // repo.commit_signed(u, signature, None);

                if imported_subcount > 0 {
                    let mut index = repo.index()?;
                    let oid = index.write_tree()?;
                    let parent_commit = repo.head()?.peel_to_commit()?;
                    let tree = repo.find_tree(oid)?;
                    let sig = git2::Signature::now("autoschematic", "import@autoschematic.sh")?;
                    let message = format!("autoschematic import by @{comment_username}: {comment_url}");
                    repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&parent_commit])?;

                    let mut remote = repo.find_remote("origin")?;

                    let refspec = format!("refs/heads/{}:refs/heads/{}", self.head_ref, self.head_ref);

                    let mut callbacks = RemoteCallbacks::new();
                    callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                        // Typically, GitHub expects:
                        //   - Username: "x-access-token"
                        //   - Password: "<YOUR_TOKEN>"
                        Cred::userpass_plaintext("x-access-token", self.token.expose_secret())
                    });

                    let mut push_options = PushOptions::new();
                    push_options.remote_callbacks(callbacks);
                    remote.push::<&str>(&[&refspec], Some(&mut push_options))?;
                }
            }
        }

        finish_run(&trace_handle).await?;

        Ok((imported_count, total_count))
    }
}
