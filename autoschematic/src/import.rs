use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
};

use autoschematic_core::{
    config::Connector,
    git_util::git_add,
    util::{load_autoschematic_config, repo_root},
    workflow::import::ImportMessage,
};
use crossterm::style::Stylize;
use dialoguer::{Confirm, MultiSelect};
use tokio::{sync::Semaphore, task::JoinSet};

use crate::CONNECTOR_CACHE;

pub async fn import(
    prefix: Option<String>,
    connector: Option<String>,
    subpath: Option<String>,
    overwrite: bool,
) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;
    let config = Arc::new(config);

    let subpath = subpath.map(PathBuf::from);

    let keystore = None;

    let mut prefix_selections: Vec<String> = vec![];

    let mut total_connectors = 0;

    // If the user didn't specify a prefix, and there are multiple prefixes, present them with the selection
    if let Some(prefix) = prefix {
        prefix_selections.push(prefix);
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

    println!(" Starting import. This may take a while!");

    let repo_root = repo_root()?;
    let mut wrote_files = false;

    let mut connector_joinset: JoinSet<anyhow::Result<Vec<PathBuf>>> = JoinSet::new();

    for (prefix_name, connector_names) in connector_selections {
        for connector_name in connector_names {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
            let reader_handle: tokio::task::JoinHandle<anyhow::Result<Vec<PathBuf>>> = {
                let prefix_name = prefix_name.clone();
                let connector_name = connector_name.clone();
                tokio::spawn(async move {
                    let mut written_files = Vec::new();
                    while let Some(msg) = receiver.recv().await {
                        match msg {
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
                            ImportMessage::StartGet { .. } => {}
                            ImportMessage::GetSuccess { addr, .. } => {
                                eprintln!(
                                    " {} Imported {}/{}",
                                    "⋉".bold(),
                                    &prefix_name.clone().dark_grey(),
                                    addr.display()
                                )
                            }
                            ImportMessage::WroteFile { path } => {
                                written_files.push(path);
                            }
                            ImportMessage::NotFound { .. } => {}
                        }
                    }
                    Ok(written_files)
                })
            };

            let config = config.clone();
            let prefix_name = prefix_name.clone();

            let keystore = keystore.clone();
            let subpath = subpath.clone();
            connector_joinset.spawn(async move {
                let semaphore = Arc::new(Semaphore::new(10));
                let _import_counts = autoschematic_core::workflow::import::import_all(
                    config,
                    CONNECTOR_CACHE.clone(),
                    keystore,
                    sender,
                    Some(semaphore),
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
        let written_files = res??;
        if !written_files.is_empty() {
            wrote_files = true;
            for path in written_files {
                git_add(&repo_root, &path)?;
            }
        }
    }

    println!("{}", " Success!".dark_green());

    if wrote_files {
        let do_commit = Confirm::new()
            .with_prompt(" ◈ Import succeeded! Do you wish to run git commit to track the imported files?")
            .default(true)
            .interact()
            .unwrap();

        if do_commit {
            Command::new("git")
                .arg("commit")
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .output()
                .expect("git commit: failed to execute");
        }
    }

    CONNECTOR_CACHE.clear().await;

    Ok(())
}
