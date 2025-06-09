use std::sync::Arc;

use anyhow::{bail, Context};
use autoschematic_core::{lockfile::AutoschematicLockfile, util::{repo_root, RON}};
use dialoguer::Select;
use regex::Regex;

use crate::{sso::load_github_token, validate::validate};

pub async fn install(url: &str, version: Option<String>) -> anyhow::Result<()> {
    let config_path = repo_root()?.join("autoschematic.ron");
    let lock_path = repo_root()?.join("autoschematic.lock");

    // Ensure autoschematic.ron is valid
    validate().context("Reading autoschematic.ron")?;

    let re = Regex::new(
        r"^(?:git@|https:\/\/)github.com[:/](?<owner>[\w\.-]+)\/(?<repo>[\w\.-]+)(.git)?$",
    )?;

    let Some(caps) = re.captures(url) else {
        bail!(
            "Error: {} doesn't appear to be a valid Github repository URL.",
            url
        )
    };


    let client = match load_github_token()? {
        Some(access_token) => {
            Arc::new(octocrab::OctocrabBuilder::new()
            .personal_token(access_token.into_secret())
            .build()?)
        }
        None => octocrab::instance()
    };

    let tag = match version {
        Some(version) => version,
        None => {
            let releases = client
                .repos(&caps["owner"], &caps["repo"])
                .releases()
                .list()
                .per_page(100)
                .send()
                .await.context("Failed to get releases \n(Hint: if this is a private repo, you may need to log in.)\n")?;

            let release_tags: Vec<&String> = releases.items.iter().map(|r| &r.tag_name).collect();

            let selection = Select::new()
                .with_prompt("Select version")
                .items(&release_tags)
                .max_length(10)
                .interact()
                .unwrap();

            release_tags.get(selection).unwrap().to_string()
        }
    };

    let release = client
        .repos(&caps["owner"], &caps["repo"])
        .releases()
        .get_by_tag(&tag)
        .await?;

    let asset_names: Vec<&String> = release.assets.iter().map(|a| &a.name).collect();
    let selection = Select::new()
        .with_prompt("Select asset")
        .items(&asset_names)
        .max_length(10)
        .interact()
        .unwrap();
    
    let asset_name = asset_names.get(selection).unwrap().to_string();

    let lock_file: AutoschematicLockfile = match lock_path.is_file() {
        true => RON.from_str(&std::fs::read_to_string(lock_path).context("Reading autoschematic.lock")?)?,
        false => AutoschematicLockfile::default(),
    };

    // Use a github client to get the manifest from the repository.
    // let manifest: ConnectorManifest = ron::from_str(s)?;
    // lock_file.entries.insert(manifest.n, v)?;

    Ok(())
}
