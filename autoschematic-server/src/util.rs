use std::{env, path::Path};

use actix_web::http::header::HeaderValue;
use anyhow::{Context, bail};
use hmac::Mac;
// use nix::fcntl::{Flock, FlockArg};
use regex::Regex;
use secrecy::ExposeSecret;
use sha2::Sha256;

use crate::{GITHUB_CRED_STORE, github_cred_store::get_github_cred_store};

pub async fn validate_github_hmac(payload: &[u8], signature: &HeaderValue) -> anyhow::Result<()> {
    let cred = get_github_cred_store().await?.read().await;

    let Some(webhook_secret) = cred.webhook_secret.as_ref().map(|s| s.expose_secret()) else {
        bail!("Webhook disabled!");
    };

    let sig_components: Vec<&str> = signature.to_str()?.split("=").collect();
    if sig_components.len() != 2 {
        bail!("Invalid Github webhook signature");
    }

    let Some(signature_type) = sig_components.first() else {
        bail!("Invalid Github webhook signature");
    };

    if *signature_type != "sha256" {
        bail!("Invalid Github webhook signature");
    }

    let Some(signature_hex) = sig_components.get(1) else {
        bail!("Invalid Github webhook signature");
    };

    let signature_value = hex::decode(signature_hex)?;

    let mut mac = hmac::Hmac::<Sha256>::new_from_slice(webhook_secret.as_bytes())?;

    mac.update(payload);
    mac.verify_slice(&signature_value)?;

    Ok(())
}

// pub fn hold_flock_nonblocking(path: &Path) -> anyhow::Result<Flock<File>> {
//     let file = OpenOptions::new().create(true).write(true).open(path)?;

//     let lock = match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
//         Ok(l) => l,
//         Err((_, e)) => return Err(e.into()),
//     };

//     Ok(lock)
// }

/// Delete an entire prefix from the filesystem.
/// Only useful in test environments!
pub async fn clear_prefix(prefix: &Path) -> anyhow::Result<()> {
    tracing::error!("Clearing prefix {:?}", prefix);
    tokio::fs::create_dir_all(prefix).await.context("create_dir")?;
    tokio::fs::remove_dir_all(prefix).await.context("remove dir")?;
    tokio::fs::create_dir_all(prefix).await.context("create_dir")
}

/// Delete an entire prefix from the filesystem, but preserve the .outputs directory.
/// Only useful in test environments!
pub async fn clear_prefix_keep_outputs(prefix: &Path) -> anyhow::Result<()> {
    tracing::error!("Clearing prefix {:?} but keeping .outputs", prefix);

    // Ensure the prefix directory exists
    tokio::fs::create_dir_all(prefix).await.context("create_dir")?;

    for entry in walkdir::WalkDir::new(prefix)
        .max_depth(1)
        .min_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path() != prefix.join(".outputs"))
    {
        if entry.path().is_dir() {
            tracing::warn!("Deleting directory {}...", entry.path().to_string_lossy());
            tokio::fs::remove_dir_all(entry.path()).await.context("remove dir")?;
        } else if entry.path().is_file() {
            tracing::warn!("Deleting file {}...", entry.path().to_string_lossy());
            tokio::fs::remove_file(entry.path()).await.context("remove file")?;
            // tokio::fs::remove_dir_all(prefix.join(entry.path()))
            //     .await
            //     .context("remove dir")?;
        }
    }

    Ok(())
}

pub fn extract_template_message_type(comment_body: &str) -> anyhow::Result<Option<String>> {
    let re = Regex::new(r"^<!--- \[(?<type>[^\]]+)\] -->")?;
    let Some(caps) = re.captures(comment_body) else {
        return Ok(None);
    };
    Ok(Some(caps["type"].to_string()))
}
