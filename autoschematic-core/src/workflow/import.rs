use std::{
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync,
};

use anyhow::{Context, bail};
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

use crate::{
    config::AutoschematicConfig,
    connector::{Connector, parse::connector_shortname},
    connector_cache::ConnectorCache,
    connector_util::build_out_path,
    error::AutoschematicError,
    glob::addr_matches_filter,
    keystore::KeyStore,
    write_output::{link_phy_output_file, write_virt_output_file},
};

pub async fn import_resource(
    connector_shortname: &str,
    connector: &Box<dyn Connector>,
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

    let phy_out_path = build_out_path(prefix, phy_addr);

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

        // let mut have_virt_addr = false;
        // let virt_addr = if phy_out_path.is_symlink() {
        //     have_virt_addr = true;
        //     unbuild_out_path(prefix, &fs::read_link(phy_out_path)?)?
        // } else {
        //     phy_addr.to_path_buf()
        // };
        let Some(virt_addr) = connector.addr_phy_to_virt(phy_addr).await? else {
            bail!("Couldn't resolve phy addr to virt: {:?}", phy_addr)
        };

        match connector
            .get(phy_addr)
            .await
            .context(format!("{}::get()", connector_shortname))?
        {
            // Dump the found resource to a string and commit and push!
            Some(get_resource_output) => {
                let body = get_resource_output.resource_definition;
                let res_path = prefix.join(&virt_addr);

                if let Some(parent) = res_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                // tokio::fs::wr
                //
                eprintln!("\u{1b}[92m [PULL] \u{1b}[39m {}", res_path.display());
                tokio::fs::write(&res_path, body).await?;

                // let mut index = repo.index()?;

                // index.add_all([res_path], IndexAddOption::default(), None)?;
                // index.write()?;

                if let Some(outputs) = get_resource_output.outputs {
                    if outputs.len() > 0 {
                        let virt_output_path = build_out_path(prefix, &virt_addr);
                        let phy_output_path = build_out_path(prefix, &phy_addr);

                        if let Some(_virt_output_path) = write_virt_output_file(&virt_output_path, &outputs, true)? {
                            // self.git_add(repo, &virt_output_path)?;
                        }

                        // TODO can import ever delete/unlink an output file?
                        if virt_addr != phy_addr {
                            if let Some(_phy_output_path) = link_phy_output_file(&virt_output_path, &phy_output_path)? {
                                // self.git_add(repo, &phy_output_path)?;
                            }
                            // let phy_output_path = build_out_path(prefix, &phy_addr);
                        }
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

pub async fn import_complete() {}

pub async fn import_all(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<&Box<dyn KeyStore>>,
    subpath: Option<PathBuf>,
    prefix_filter: Option<String>,
    connector_filter: Option<String>,
    overwrite_existing: bool,
) -> Result<(usize, usize), AutoschematicError> {
    let resource_group_map = autoschematic_config.resource_group_map();

    let subpath = subpath.unwrap_or(PathBuf::from("./"));

    // Number of resources found and imported
    let mut imported_count: usize = 0;
    // Number of resources found
    let mut total_count: usize = 0;

    for (prefix_name, prefix) in &autoschematic_config.prefixes {
        if let Some(prefix_filter) = &prefix_filter {
            if prefix_name != prefix_filter {
                continue;
            }
        }
        for connector_def in &prefix.connectors {
            let prefix_name = PathBuf::from(&prefix_name);

            let connector_shortname = connector_shortname(&connector_def.name)?;
            if let Some(connector_filter) = &connector_filter {
                if connector_shortname != *connector_filter {
                    continue;
                }
            }
            // subcount represents the number of resources imported by this connector,
            // count represents the number of resources imported by all connectors
            let mut imported_subcount: usize = 0;
            // temporarily cd into the repo...
            // I.E. `cd /tmp/autoschematic-298347928/pfnsec/autoschematic-playground`
            // let _chwd = self.chwd_to_repo();

            eprintln!("connector init: {}", connector_def.name);
            let (connector, mut inbox) = connector_cache
                .get_or_spawn_connector(
                    &connector_def.name,
                    &PathBuf::from(&prefix_name),
                    &connector_def.env,
                    keystore,
                )
                .await?;
            // let sender_trace_handle = trace_handle.clone();
            let _reader_handle = tokio::spawn(async move {
                loop {
                    match inbox.recv().await {
                        Ok(Some(stdout)) => {
                            eprintln!("{}", stdout);
                            // let res = append_run_log(&sender_trace_handle, stdout).await;
                            // match res {
                            //     Ok(_) => {}
                            //     Err(_) => {}
                            // }
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
                if !addr_matches_filter(&prefix_name, &phy_addr, &subpath) {
                    continue 'phy_addr;
                }

                // Skip files that already exist in other resource groups.
                if let Some(ref resource_group) = prefix.resource_group {
                    if let Some(neighbour_prefixes) = resource_group_map.get(resource_group) {
                        // get all prefixes in this resource group except our own
                        for neighbour_prefix in neighbour_prefixes.iter().filter(|p| **p != prefix_name) {
                            if neighbour_prefix.join(&phy_addr).exists() {
                                continue 'phy_addr;
                            }

                            if build_out_path(neighbour_prefix, &phy_addr).exists() {
                                continue 'phy_addr;
                            }
                        }
                    }
                }

                let res =
                    import_resource(&connector_shortname, &connector, &prefix_name, &phy_addr, overwrite_existing).await?;
                if res {
                    imported_subcount += 1;
                    imported_count += 1;
                }
                total_count += 1;
            }
        }
    }

    Ok((imported_count, total_count))
}
