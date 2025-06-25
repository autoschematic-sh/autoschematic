use std::{
    path::{Component, PathBuf},
};

use anyhow::bail;
use dialoguer::{Confirm, Input, Select};
use regex::Regex;

use autoschematic_core::{
    connector::{FilterOutput, parse::connector_shortname},
    connector_cache::ConnectorCache,
    util::repo_root,
    workflow,
};

use crate::config::load_autoschematic_config;

pub async fn create(prefix: &Option<String>, connector: &Option<String>) -> anyhow::Result<()> {
    let config = load_autoschematic_config()?;

    let connector_cache = ConnectorCache::default();

    let prefix_names: Vec<&String> = config.prefixes.iter().map(|a| a.0).collect();
    let prefix_i = Select::new()
        .with_prompt("Select prefix")
        .items(&prefix_names)
        .max_length(10)
        .interact()
        .unwrap();

    let prefix = prefix_names.get(prefix_i).unwrap().to_owned();

    let prefix_def = config.prefixes.get(prefix).unwrap();

    let connector_names: Vec<String> = prefix_def.connectors.iter().map(|c| c.shortname.clone()).collect();
    let connector_i = Select::new()
        .with_prompt("Select connector")
        .items(&connector_names)
        .max_length(10)
        .interact()
        .unwrap();

    let connector_name = connector_names.get(connector_i).unwrap();

    let connector_def = prefix_def.connectors.get(connector_i).unwrap();

    let skeletons =
        workflow::get_skeletons::get_skeletons(&config, &connector_cache, None, &PathBuf::from(prefix), connector_def).await?;

    let skeleton_paths: Vec<String> = skeletons.iter().map(|a| a.addr.to_str().unwrap().to_string()).collect();
    let skeleton_i = Select::new()
        .with_prompt("Select object")
        .items(&skeleton_paths)
        .max_length(10)
        .interact()
        .unwrap();

    let skeleton = skeletons.get(skeleton_i).unwrap();

    let walk_path = repo_root()?.join(prefix);

    let mut output_addr = skeleton.addr.to_str().unwrap().to_string();

    for component in skeleton.addr.components() {
        if let Component::Normal(dir) = component {
            let dir = dir.to_str().unwrap();

            let re = Regex::new(r"\[(?<template>[^\[\]]+)\]")?;

            if let Some(caps) = re.captures(dir) {
                let var_name = &caps["template"];
                let var: String = Input::new().with_prompt(var_name.to_string()).interact_text().unwrap();

                output_addr = output_addr.replace(&format!("[{}]", var_name), &var);
            }
        }
    }

    let output_path = PathBuf::from(output_addr);

    let prefix = PathBuf::from(prefix);

    if prefix.join(&output_path).exists() {
        bail!("Error: output path {} already exists.", output_path.display())
    }

    if workflow::filter::filter(&config, &connector_cache, None, &prefix, &output_path).await? == FilterOutput::None {
        let write_override = Confirm::new()
            .with_prompt(
                format!("The resulting output path:\n {}\n didn't match any enabled connectors. This file won't do anything.\nWant to write to it anyway?",
                prefix.join(&output_path).display()
            ))
            .default(false)
            .interact()
            .unwrap();
        if !write_override {
            return Ok(());
        }
    }

    println!("Writing {}/{}", prefix.display(), output_path.display());

    tokio::fs::create_dir_all(prefix.join(&output_path).parent().unwrap()).await?;

    tokio::fs::write(prefix.join(output_path), &skeleton.body).await?;

    Ok(())
}
