use std::{collections::HashMap, path::PathBuf};

use autoschematic_core::{
    connector::{self, parse::connector_shortname},
    connector_cache::ConnectorCache,
};
use crossterm::style::Stylize;
use dialoguer::MultiSelect;

use crate::config::load_autoschematic_config;

pub async fn import(
    prefix: Option<String>,
    connector: Option<String>,
    subpath: Option<String>,
    overwrite: bool,
) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let connector_cache = ConnectorCache::default();

    let subpath = subpath.map(PathBuf::from);

    let keystore = None;

    let mut prefix_selections: Vec<String> = vec![];

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

    // TODO handle the CLI connector filter also
    let mut connector_selections: HashMap<String, Vec<String>> = HashMap::new();
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
        }

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
            // prefix_selections.push(items[i].to_string());
        }
    }

    eprintln!("Starting import. This may take a while!");

    for (prefix_name, connector_names) in connector_selections {
        for connector_name in connector_names {
            let import_counts = autoschematic_core::workflow::import::import_all(
                &config,
                &connector_cache,
                keystore,
                subpath.clone(),
                Some(prefix_name.clone()),
                Some(connector_name.clone()),
                overwrite,
            )
            .await?;
        }
    }

    eprintln!("\u{1b}[32m Success! \u{1b}[39m");

    Ok(())
}
