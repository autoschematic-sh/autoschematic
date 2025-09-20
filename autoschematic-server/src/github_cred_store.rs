use std::{env, path::PathBuf};

use anyhow::bail;
use cocoon::CocoonCipher;
use once_cell::sync::OnceCell;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    credentials::load_github_private_key,
    error::{self, AutoschematicServerErrorType},
};
pub static GITHUB_CRED_STORE: OnceCell<RwLock<GithubCredStore>> = OnceCell::new();

pub async fn get_github_cred_store() -> Result<&'static RwLock<GithubCredStore>, error::AutoschematicServerError> {
    let Some(cred_store) = GITHUB_CRED_STORE.get() else {
        return Err(error::AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(anyhow::anyhow!("Github Cred Store unset")),
        }
        .into());
    };

    Ok(cred_store)
}

#[derive(Debug)]
pub struct GithubCredStore {
    pub set_by_manifest: bool,
    pub app_name: Option<String>,
    pub app_slug: Option<String>,
    pub webhook_secret: Option<SecretString>,
    pub client_id: Option<SecretString>,
    pub client_secret: Option<SecretString>,
    pub private_key: Option<SecretString>,
}

#[derive(Serialize, Deserialize)]
pub struct GithubCredStoreFile {
    pub app_name: String,
    pub app_slug: String,
    pub webhook_secret: String,
    pub client_id: String,
    pub client_secret: String,
    pub private_key: String,
}

impl GithubCredStore {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(GithubCredStore {
            set_by_manifest: false,
            app_name: env::var("GITHUB_APP_NAME").ok().map(|w| w.into()),
            app_slug: env::var("GITHUB_APP_SLUG").ok().map(|w| w.into()),
            client_id: env::var("GITHUB_CLIENT_ID").ok().map(|w| w.into()),
            webhook_secret: env::var("WEBHOOK_SECRET").ok().map(|w| w.into()),
            client_secret: env::var("GITHUB_CLIENT_SECRET").ok().map(|w| w.into()),
            private_key: Some(load_github_private_key().await?.into()),
        })
    }

    pub fn from_manifest_result(&mut self, value: &serde_json::Value) -> anyhow::Result<()> {
        if self.set_by_manifest {
            bail!("Github: Manifest credentials already set")
        }

        self.set_by_manifest = true;

        self.app_name = value.get("name").and_then(|v| v.as_str()).map(|v| v.into());
        self.app_slug = value.get("slug").and_then(|v| v.as_str()).map(|v| v.into());
        self.webhook_secret = value.get("webhook_secret").and_then(|v| v.as_str()).map(|v| v.into());
        self.client_id = value.get("client_id").and_then(|v| v.as_str()).map(|v| v.into());
        self.client_secret = value.get("client_secret").and_then(|v| v.as_str()).map(|v| v.into());
        self.private_key = value.get("private_key").and_then(|v| v.as_str()).map(|v| v.into());

        Ok(())
    }

    pub async fn save(&self) -> anyhow::Result<()> {
        let cred_file: GithubCredStoreFile = self.into();

        cred_file.save().await?;

        Ok(())
    }
}

impl From<GithubCredStoreFile> for GithubCredStore {
    fn from(value: GithubCredStoreFile) -> Self {
        Self {
            set_by_manifest: true,
            app_name: Some(value.app_name.into()),
            app_slug: Some(value.app_slug.into()),
            webhook_secret: Some(value.webhook_secret.into()),
            client_id: Some(value.client_id.into()),
            client_secret: Some(value.client_secret.into()),
            private_key: Some(value.private_key.into()),
        }
    }
}

#[rustfmt::skip]
impl From<&GithubCredStore> for GithubCredStoreFile {
    fn from(value: &GithubCredStore) -> Self {
        Self {
            app_name: value.app_name.as_ref().cloned().unwrap_or_default(),
            app_slug: value.app_slug.as_ref().cloned().unwrap_or_default(),
            webhook_secret: value.webhook_secret.as_ref().map(|s| s.expose_secret().into()).unwrap_or_default(),
            client_id: value.client_id.as_ref().map(|s| s.expose_secret().into()).unwrap_or_default(),
            client_secret: value.client_secret.as_ref().map(|s| s.expose_secret().into()).unwrap_or_default(),
            private_key: value.private_key.as_ref().map(|s| s.expose_secret().into()).unwrap_or_default(),
        }
    }
}

const CRED_PATH: &'static str = ".autoschematic.github.cred.json";

impl GithubCredStoreFile {
    pub async fn save(&self) -> anyhow::Result<()> {
        // TODO also support loading the key from kms/secrets/etc
        let Ok(key) = env::var("GITHUB_CRED_PASSWORD") else {
            bail!("GITHUB_CRED_PASSWORD unset, can't save GithubCredStoreFile");
        };

        let mut cocoon = cocoon::Cocoon::new(key.as_bytes());
        match cocoon.wrap(serde_json::to_string(self)?.as_bytes()) {
            Ok(wrapped) => {
                tokio::fs::write(CRED_PATH, wrapped).await?;
            }
            Err(e) => {
                bail!("Failed to wrap encrypted github cred store: {:#?}", e)
            }
        }

        Ok(())
    }

    pub async fn load() -> anyhow::Result<Option<Self>> {
        if !PathBuf::from(CRED_PATH).exists() {
            return Ok(None);
        }

        // TODO also support loading the key from kms/secrets/etc
        let Ok(key) = env::var("GITHUB_CRED_PASSWORD") else {
            bail!("GITHUB_CRED_PASSWORD unset, can't save GithubCredStoreFile");
        };

        let cocoon = cocoon::Cocoon::new(key.as_bytes());

        let body = tokio::fs::read(CRED_PATH).await?;

        match cocoon.unwrap(&body) {
            Ok(unwrapped) => return Ok(Some(serde_json::from_slice(&unwrapped)?)),
            Err(e) => {
                bail!("Failed to wrap encrypted github cred store: {:#?}", e)
            }
        }
    }
}
