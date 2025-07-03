use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use tokio::{
    sync::{Semaphore, broadcast::error::RecvError},
    task::JoinSet,
};

use crate::{
    config::AutoschematicConfig,
    connector::{Connector, OutputMapFile},
    connector_cache::ConnectorCache,
    error::AutoschematicError,
    glob::addr_matches_filter,
    keystore::KeyStore,
};

#[derive(Debug)]
pub enum ImportMessage {
    StartImport { subpath: PathBuf },
    SkipExisting { prefix: PathBuf, addr: PathBuf },
    StartGet { prefix: PathBuf, addr: PathBuf },
    WroteFile { path: PathBuf },
    GetSuccess { prefix: PathBuf, addr: PathBuf },
    NotFound { prefix: PathBuf, addr: PathBuf },
}

pub type ImportOutbox = tokio::sync::mpsc::Sender<ImportMessage>;
pub type ImportInbox = tokio::sync::mpsc::Sender<ImportMessage>;

pub async fn import_resource(
    connector_shortname: &str,
    connector: Arc<dyn Connector>,
    outbox: ImportOutbox,
    prefix: &Path,
    phy_addr: &Path,
    overwrite_existing: bool,
) -> anyhow::Result<()> {
    let phy_addr = if phy_addr.is_absolute() {
        phy_addr.strip_prefix("/")?
    } else {
        phy_addr
    };

    let virt_addr = connector.addr_phy_to_virt(phy_addr).await?.unwrap_or(phy_addr.to_path_buf());

    let phy_path = prefix.join(phy_addr);
    let phy_out_path = OutputMapFile::path(prefix, phy_addr);
    let virt_out_path = OutputMapFile::path(prefix, &virt_addr);
    let virt_path = prefix.join(&virt_addr);

    if virt_path.exists() && !overwrite_existing {
        outbox
            .send(ImportMessage::SkipExisting {
                prefix: prefix.to_path_buf(),
                addr: virt_addr.to_path_buf(),
            })
            .await?;
    } else if phy_path.exists() && !overwrite_existing {
        outbox
            .send(ImportMessage::SkipExisting {
                prefix: prefix.to_path_buf(),
                addr: phy_addr.to_path_buf(),
            })
            .await?;
    } else if phy_out_path.exists() && !overwrite_existing {
        outbox
            .send(ImportMessage::SkipExisting {
                prefix: prefix.to_path_buf(),
                addr: phy_addr.to_path_buf(),
            })
            .await?;
    } else if virt_out_path.exists() && !overwrite_existing {
        outbox
            .send(ImportMessage::SkipExisting {
                prefix: prefix.to_path_buf(),
                addr: virt_addr.to_path_buf(),
            })
            .await?;
    } else {
        tracing::info!("import at path: {:?}", virt_path);

        match connector
            .get(phy_addr)
            .await
            .context(format!("{connector_shortname}::get()"))?
        {
            Some(get_resource_output) => {
                outbox
                    .send(ImportMessage::GetSuccess {
                        prefix: prefix.to_path_buf(),
                        addr: virt_addr.to_path_buf(),
                    })
                    .await?;

                let wrote_files = get_resource_output.write(prefix, phy_addr, &virt_addr).await?;
                for wrote_file in wrote_files {
                    outbox.send(ImportMessage::WroteFile { path: wrote_file }).await?;
                }
                // let body = get_resource_output.resource_definition;
                // let res_path = prefix.join(&virt_addr);

                // if let Some(parent) = res_path.parent() {
                //     tokio::fs::create_dir_all(parent).await?;
                // }
                // // tokio::fs::wr
                // //
                // eprintln!("\u{1b}[92m [PULL] \u{1b}[39m {}", res_path.display());
                // tokio::fs::write(&res_path, body).await?;

                // // let mut index = repo.index()?;

                // // index.add_all([res_path], IndexAddOption::default(), None)?;
                // // index.write()?;

                // if let Some(outputs) = get_resource_output.outputs {
                //     if !outputs.is_empty() {
                //         let output_map_file = OutputMapFile::OutputMap(outputs);
                //         output_map_file.write(prefix, &virt_addr)?;

                //         if virt_addr != phy_addr {
                //             OutputMapFile::write_link(prefix, phy_addr, &virt_addr)?;
                //         }
                //     }
                // }

                // return Ok(true);
            }
            None => {
                outbox
                    .send(ImportMessage::NotFound {
                        prefix: prefix.to_path_buf(),
                        addr: phy_addr.to_path_buf(),
                    })
                    .await?;
                tracing::error!("No remote resource at addr:{:?} path: {:?}", phy_addr, virt_path);
                // TODO bail on an error here, this indicates a probable connector bug!
            }
        }
    }
    Ok(())
}

pub async fn import_complete() {}

pub async fn import_all(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    outbox: ImportOutbox,
    semaphore: Arc<Semaphore>,
    subpath: Option<PathBuf>,
    prefix_filter: Option<String>,
    connector_filter: Option<String>,
    overwrite_existing: bool,
) -> Result<(usize, usize), AutoschematicError> {
    let resource_group_map = autoschematic_config.resource_group_map();

    let subpath = subpath.unwrap_or(PathBuf::from("./"));

    // Number of resources found and imported
    let imported_count: usize = 0;
    // Number of resources found
    let total_count: usize = 0;

    // Represents the joinset for each list() operation.
    let mut subpath_joinset: JoinSet<anyhow::Result<Vec<PathBuf>>> = JoinSet::new();

    for (prefix_name, prefix) in &autoschematic_config.prefixes {
        if let Some(prefix_filter) = &prefix_filter
            && prefix_name != prefix_filter {
                continue;
            }
        for connector_def in &prefix.connectors {
            let prefix_name = PathBuf::from(&prefix_name);

            if let Some(connector_filter) = &connector_filter
                && connector_def.shortname != *connector_filter {
                    continue;
                }
            // subcount represents the number of resources imported by this connector,
            // count represents the number of resources imported by all connectors
            let imported_subcount: usize = 0;

            tracing::info!("connector init: {}", connector_def.shortname);
            let (connector, mut inbox) = connector_cache
                .get_or_spawn_connector(
                    &connector_def.shortname,
                    &connector_def.spec,
                    &PathBuf::from(&prefix_name),
                    &connector_def.env,
                    keystore.clone(),
                )
                .await?;
            let _reader_handle = tokio::spawn(async move {
                loop {
                    match inbox.recv().await {
                        Ok(Some(stdout)) => {
                            eprintln!("{stdout}");
                        }
                        Err(RecvError::Closed) => break,
                        _ => {}
                    }
                }
            });

            let connector_subpaths = connector
                .subpaths()
                .await
                .context(format!("{}::subpaths()", connector_def.shortname,))?;

            // Represents the joinset for each import operation.
            let mut import_joinset: JoinSet<anyhow::Result<()>> = JoinSet::new();

            for connector_subpath in connector_subpaths {
                // Here, we convert the requested subpath into the orthogonal
                // sub-address-space as supported by the connector - but only
                // if the requested subpath is within that subspace.
                if !(addr_matches_filter(&connector_subpath, &subpath) || addr_matches_filter(&subpath, &connector_subpath)) {
                    continue;
                }

                outbox
                    .send(ImportMessage::StartImport {
                        subpath: connector_subpath.clone(),
                    })
                    .await?;

                let connector_shortname = connector_def.shortname.clone();
                let subpath_connector = connector.clone();
                subpath_joinset.spawn(async move {
                    subpath_connector.list(&connector_subpath).await.context(format!(
                        "{}::list({})",
                        connector_shortname,
                        connector_subpath.display()
                    ))
                });

                while let Some(res) = subpath_joinset.join_next().await {
                    let phy_addrs = res??;
                    'phy_addr: for phy_addr in phy_addrs {
                        if !addr_matches_filter(&phy_addr, &subpath) {
                            continue 'phy_addr;
                        }

                        // Skip files that already exist in other resource groups.
                        if let Some(ref resource_group) = prefix.resource_group
                            && let Some(neighbour_prefixes) = resource_group_map.get(resource_group) {
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

                        let prefix_name = prefix_name.clone();
                        let outbox = outbox.clone();
                        let connector_shortname = connector_def.shortname.clone();
                        let connector = connector.clone();
                        // let autoschematic_config = autoschematic_config.clone();
                        // let keystore = keystore.clone();
                        import_joinset.spawn(async move {
                            import_resource(
                                &connector_shortname,
                                connector,
                                outbox,
                                &prefix_name,
                                &phy_addr,
                                overwrite_existing,
                            )
                            .await?;
                            Ok(())
                        });
                    }
                }
            }

            while let Some(res) = import_joinset.join_next().await {}
        }
    }

    while let Some(res) = subpath_joinset.join_next().await {}

    Ok((imported_count, total_count))
}
