use std::{collections::HashMap, path::PathBuf, sync::Arc};

use autoschematic_core::{config::Connector, connector_cache::ConnectorCache, workflow::import::ImportMessage};
use crossterm::style::Stylize;
use dialoguer::MultiSelect;
use tokio::{sync::Semaphore, task::JoinSet};

use crate::config::load_autoschematic_config;

pub async fn import(
    prefix: Option<String>,
    connector: Option<String>,
    subpath: Option<String>,
    overwrite: bool,
) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;
    let config = Arc::new(config);

    let connector_cache = Arc::new(ConnectorCache::default());

    let subpath = subpath.map(PathBuf::from);

    let keystore = None;

    let mut prefix_selections: Vec<String> = vec![];

    let mut total_connectors = 0;

    // If the user didn't specify a prefix, and there are multiple prefixes, present them with the selection
    if prefix.is_some() {
        prefix_selections.push(prefix.unwrap());
    } else if config.prefixes.len() == 1 {
        prefix_selections.push(config.prefixes.keys().collect::<Vec<&String>>().first().unwrap().to_string());
    } else if prefix.is_none() && config.prefixes.len() > 1 {
        let items: Vec<&String> = config.prefixes.keys().collect();

        let selection = MultiSelect::new()
            .with_prompt(" ⊆ In which prefixes should we query and import remote resources?")
            .items(&items)
            .interact()
            .unwrap();

        for i in selection {
            prefix_selections.push(items[i].to_string());
        }
    }

    let mut connector_selections: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(connector_selection) = connector {
        for prefix_selection in &prefix_selections {
            connector_selections.insert(prefix_selection.clone(), Vec::new());

            let connectors: &Vec<Connector> = &config.prefixes.get(prefix_selection).unwrap().connectors;
            for prefix_connector in connectors {
                if prefix_connector.shortname == connector_selection {
                    connector_selections
                        .get_mut(prefix_selection)
                        .unwrap()
                        .push(connector_selection.to_string());

                    total_connectors += 1;
                }
            }
        }
    } else {
        for prefix_selection in &prefix_selections {
            connector_selections.insert(prefix_selection.clone(), Vec::new());

            let items: Vec<String> = config
                .prefixes
                .get(prefix_selection)
                .unwrap()
                .connectors
                .iter()
                .map(|c| c.shortname.clone())
                .collect();

            if items.is_empty() {
                continue;
            }

            if items.len() == 1 {
                connector_selections
                    .get_mut(prefix_selection)
                    .unwrap()
                    .push(items.first().unwrap().to_string());
                total_connectors += 1;
            } else {
                let selection = MultiSelect::new()
                    .with_prompt(format!(
                        " ⊆ In prefix {}, with which connectors should we query and import remote resources?",
                        prefix_selection.clone().dark_grey(),
                    ))
                    .items(&items)
                    .interact()
                    .unwrap();

                for i in selection {
                    connector_selections
                        .get_mut(prefix_selection)
                        .unwrap()
                        .push(items[i].to_string());
                    total_connectors += 1;
                }
            }
        }
    }

    if total_connectors == 0 {
        println!(" ∅ Selection matched no connectors, or your prefix(es) are empty.");
        return Ok(());
    }

    println!("{}", " Starting import. This may take a while!");

    let mut connector_joinset: JoinSet<anyhow::Result<()>> = JoinSet::new();

    for (prefix_name, connector_names) in connector_selections {
        for connector_name in connector_names {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
            let reader_handle: tokio::task::JoinHandle<anyhow::Result<()>> = {
                let prefix_name = prefix_name.clone();
                let connector_name = connector_name.clone();
                tokio::spawn(async move {
                    loop {
                        match receiver.recv().await {
                            Some(msg) => match msg {
                                ImportMessage::StartImport { subpath } => {
                                    println!(
                                        "{}: Starting import under {}/{}",
                                        &connector_name,
                                        &prefix_name.clone().dark_grey(),
                                        subpath.display()
                                    )
                                }
                                ImportMessage::SkipExisting { prefix, addr } => {
                                    eprintln!(
                                        " {} Skipping {}/{} (already exists)",
                                        "∋".dark_grey(),
                                        prefix.to_string_lossy().dark_grey(),
                                        addr.display()
                                    )
                                }
                                ImportMessage::StartGet { prefix, addr } => {}
                                ImportMessage::GetSuccess { prefix, addr } => {
                                    eprintln!(
                                        " {} Imported {}/{}",
                                        "⋉".bold(),
                                        &prefix_name.clone().dark_grey(),
                                        addr.display()
                                    )
                                }
                                ImportMessage::WroteFile { path } => {}
                                ImportMessage::NotFound { prefix, addr } => {}
                            },
                            None => break,
                        }
                    }
                    Ok(())
                })
            };

            let config = config.clone();
            let prefix_name = prefix_name.clone();
            let connector_cache = connector_cache.clone();
            let keystore = keystore.clone();
            let subpath = subpath.clone();
            connector_joinset.spawn(async move {
                let semaphore = Arc::new(Semaphore::new(10));
                let import_counts = autoschematic_core::workflow::import::import_all(
                    config,
                    connector_cache,
                    keystore,
                    sender,
                    semaphore,
                    subpath,
                    Some(prefix_name),
                    Some(connector_name.clone()),
                    overwrite,
                )
                .await?;

                reader_handle.await?
            });
        }
    }

    while let Some(res) = connector_joinset.join_next().await {
        res??
    }
    println!("{}", " Success!".dark_green());

    Ok(())
}
