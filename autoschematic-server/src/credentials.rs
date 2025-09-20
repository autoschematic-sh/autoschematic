use std::env;

use anyhow::Context;
use base64::prelude::*;
use octocrab::Octocrab;
use octocrab::models::InstallationId;
use secrecy::{ExposeSecret, SecretString};


pub async fn load_github_private_key() -> Result<String, anyhow::Error> {
    let private_key_path = env::var("GITHUB_PRIVATE_KEY_PATH");
    let private_key_base64 = env::var("GITHUB_PRIVATE_KEY_BASE64");

    match (private_key_base64, private_key_path) {
        (Ok(_), Ok(_)) => Err(anyhow::Error::msg(
            "Ambiguous: Only one of GITHUB_PRIVATE_KEY_BASE64 and GITHUB_PRIVATE_KEY_PATH can be set!",
        )),
        (Ok(private_key_base64), Err(_)) => {
            let private_key = BASE64_STANDARD.decode(private_key_base64)?;

            Ok(String::from_utf8(private_key)?)
        }
        (Err(_), Ok(private_key_path)) => Ok(std::fs::read_to_string(private_key_path)?),
        (Err(_), Err(_)) => Err(anyhow::Error::msg(
            "GITHUB_PRIVATE_KEY_BASE64 or GITHUB_PRIVATE_KEY_PATH not set!",
        )),
    }
}

pub async fn octocrab_installation_client(installation_id: InstallationId) -> anyhow::Result<(Octocrab, SecretString)> {
    let app_id = env::var("GITHUB_APP_ID").context("Missing GITHUB_APP_ID!")?;

    let private_key = load_github_private_key().await?;

    let key = jsonwebtoken::EncodingKey::from_rsa_pem(&private_key.into_bytes())?;

    let octocrab = Octocrab::builder()
        .app(octocrab::models::AppId(app_id.parse()?), key)
        .build()
        .context("Failed to build Octocrab client!")?;

    let (octocrab, token) = octocrab.installation_and_token(installation_id).await?;
    Ok((octocrab, token))
}

pub async fn octocrab_user_client(user_access_token: &str) -> anyhow::Result<Octocrab> {
    // let key = jsonwebtoken::EncodingKey::from_rsa_pem(&private_key.into_bytes())?;

    let octocrab = Octocrab::builder()
        .user_access_token(user_access_token)
        // .app(octocrab::models::AppId(app_id.parse()?), key)
        .build()
        .context("Failed to build Octocrab client!")?;

    // let (octocrab, token) = octocrab.installation_and_token(installation_id).await?;
    Ok(octocrab)
}
