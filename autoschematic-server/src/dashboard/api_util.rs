use actix_session::Session;
use anyhow::bail;
use autoschematic_core::config::AutoschematicConfig;
use octocrab::models::InstallationId;
use reqwest::Response;
use ron::error::SpannedResult;
use ron_pfnsec_fork as ron;
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};

use crate::{
    RON,
    credentials::{octocrab_installation_client, octocrab_user_client},
    error::AutoschematicServerError,
};

pub async fn get_self(access_token: &str) -> Result<Response, AutoschematicServerError> {
    let client = reqwest::Client::new();
    let res = client
        .get("https://api.github.com/user")
        .header("User-Agent", "autoschematic")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to get user from session token: {}", e);
            AutoschematicServerError::from(e)
        })?;
    // TODO can we do something nice with their avatar etc?
    Ok(res)
}

pub async fn has_valid_session(session: &Session) -> Result<Option<(String, String)>, actix_web::Error> {
    if let (Some(access_token), Some(github_username)) = (
        session.get::<String>("access_token")?,
        session.get::<String>("github_username")?,
    ) {
        let res = get_self(&access_token).await?;
        let user: serde_json::Value = res.json().await.map_err(|e| {
            tracing::error!("Couldn't parse user json {}", e);
            AutoschematicServerError::from(e)
        })?;

        let Some(username) = user.get("login") else {
            return Ok(None);
        };

        let Some(username) = username.as_str() else {
            return Ok(None);
        };

        if username != github_username {
            return Ok(None);
        }

        Ok(Some((access_token, github_username)))
    } else {
        Ok(None)
    }
}

pub async fn is_repo_collaborator(
    access_token: &str,
    username: &str,
    owner: &str,
    repo: &str,
) -> Result<bool, AutoschematicServerError> {
    let client = reqwest::Client::new();

    let url = format!("https://api.github.com/repos/{owner}/{repo}/collaborators/{username}");

    match client
        .get(url)
        .header("User-Agent", "autoschematic")
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to test if user is repo collaborator: {}", e);
            AutoschematicServerError::from(e)
        }) {
        Ok(res) => {
            let status = res.status();
            if status == reqwest::StatusCode::NO_CONTENT {
                tracing::info!("is repo collaborator: {} {} {}", username, repo, status);
                Ok(true)
            } else {
                tracing::info!("is not repo collaborator: {} {} {}", username, repo, status);
                Ok(false)
            }
        }
        Err(e) => {
            tracing::info!("is not repo collaborator: {} ", e);
            Ok(false)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PrInfo {
    pub title: String,
    pub author: String,
}

pub async fn get_pr_info(access_token: &str, owner: &str, repo: &str, pr: u64) -> Result<PrInfo, AutoschematicServerError> {
    let client = octocrab::Octocrab::builder()
        .user_access_token(access_token)
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build octocrab client: {}", e);
            AutoschematicServerError::from(e)
        })?;

    let pull_result = client.pulls(owner, repo).get(pr).await.map_err(|e| {
        tracing::error!("Failed to get pull request: {}", e);
        AutoschematicServerError::from(e)
    })?;

    let author_login = match pull_result.user {
        Some(user) => user.login,
        None => String::new(),
    };

    Ok(PrInfo {
        title: pull_result.title.unwrap_or_default(),
        author: author_login,
    })
}

#[derive(Serialize)]
pub struct InstallationInfo {
    pub owner: String,
    pub repo: String,
    pub installation_id: u64,
}

pub async fn get_installations(jwt: &SecretBox<str>) -> anyhow::Result<Vec<InstallationInfo>> {
    let client = reqwest::Client::new();

    let url = "https://api.github.com/app/installations".to_string();

    let res: serde_json::Value = client
        .get(url)
        .header("User-Agent", "autoschematic")
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {}", jwt.expose_secret()))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?
        .json()
        .await?;

    let Some(installations) = res.as_array() else {
        bail!("GET app/installations: expected array")
    };

    let mut install_results = Vec::new();
    for installation in installations {
        if let Some(Some(id)) = installation.get("id").map(|id| id.as_u64()) {
            let (install_client, install_token) = octocrab_installation_client(InstallationId(id)).await?;

            let res: serde_json::Value = client
                .get("https://api.github.com/installation/repositories")
                .header("User-Agent", "autoschematic")
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("Bearer {}", install_token.expose_secret()))
                .header("X-GitHub-Api-Version", "2022-11-28")
                .send()
                .await?
                .json()
                .await?;

            let Some(repos) = res.get("repositories").and_then(|repos| repos.as_array()) else {
                continue;
            };

            for repo in repos {
                let Some(repo_name) = repo.get("name").and_then(|name| name.as_str()) else {
                    continue;
                };
                let Some(owner) = repo
                    .get("owner")
                    .and_then(|owner| owner.get("login").and_then(|owner| owner.as_str()))
                else {
                    continue;
                };

                let repository = install_client.repos(owner, repo_name).get().await?;

                let Some(default_branch) = repository.default_branch else {
                    continue;
                };

                let Ok(mut config_content) = install_client
                    .repos(owner, repo_name)
                    .get_content()
                    .path("autoschematic.ron")
                    .r#ref(default_branch)
                    .send()
                    .await
                    .map_err(|e| {})
                else {
                    continue;
                };

                let contents = config_content.take_items();
                let c = &contents[0];
                let decoded_content = c.decoded_content().unwrap_or_default();

                let config: SpannedResult<AutoschematicConfig> = RON.from_str(&decoded_content);

                match config {
                    Ok(config) => {
                        install_results.push(InstallationInfo {
                            owner: owner.into(),
                            repo: repo_name.into(),
                            installation_id: id,
                        });
                    }
                    Err(e) => {
                        tracing::error!("Decoding autoschematic.ron: {:#?}", e)
                    }
                }
            }
        }
    }

    Ok(install_results)
}

pub async fn get_installations_for_user(user_access_token: &str) -> anyhow::Result<Vec<InstallationInfo>> {
    let client = reqwest::Client::new();

    let url = "https://api.github.com/user/installations".to_string();

    let res: serde_json::Value = client
        .get(url)
        .header("User-Agent", "autoschematic")
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {user_access_token}"))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?
        .json()
        .await?;

    let Some(installations) = res.get("installations").and_then(|i| i.as_array()) else {
        bail!("GET user/installations: expected array")
    };

    let mut install_results = Vec::new();
    for installation in installations {
        if let Some(id) = installation.get("id").and_then(|id| id.as_u64()) {
            let res: serde_json::Value = client
                .get(format!("https://api.github.com/user/installations/{id}/repositories"))
                .header("User-Agent", "autoschematic")
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("Bearer {user_access_token}"))
                .header("X-GitHub-Api-Version", "2022-11-28")
                .send()
                .await?
                .json()
                .await?;

            let Some(repos) = res.get("repositories").and_then(|repos| repos.as_array()) else {
                continue;
            };

            for repo in repos {
                let Some(repo_name) = repo.get("name").and_then(|name| name.as_str()) else {
                    continue;
                };
                let Some(owner) = repo
                    .get("owner")
                    .and_then(|owner| owner.get("login").and_then(|owner| owner.as_str()))
                else {
                    continue;
                };

                let has_admin = repo
                    .get("permissions")
                    .and_then(|perms| perms.get("admin"))
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);

                let has_push = repo
                    .get("permissions")
                    .and_then(|perms| perms.get("push"))
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);

                if has_admin || has_push {
                    install_results.push(InstallationInfo {
                        owner: owner.into(),
                        repo: repo_name.into(),
                        installation_id: id,
                    });
                }
            }
        }
        // tracing::error!("{:#?}", installation.get("target_type"))
    }

    Ok(install_results)
}

pub async fn get_config_for_repo(
    owner: &str,
    repo: &str,
    user_access_token: &str,
) -> anyhow::Result<Option<AutoschematicConfig>> {
    let user_client = octocrab_user_client(user_access_token).await?;

    let repository = user_client.repos(owner, repo).get().await?;

    let Some(default_branch) = repository.default_branch else {
        return Ok(None);
    };

    let config_content = user_client
        .repos(owner, repo)
        .get_content()
        .path("autoschematic.ron")
        .r#ref(default_branch)
        .send()
        .await;

    let contents = config_content?.take_items();
    let c = &contents[0];
    let decoded_content = c.decoded_content().unwrap_or_default();

    let config: AutoschematicConfig = RON.from_str(&decoded_content)?;

    // tracing::error!("get_content()=> {:?}", config_content);

    Ok(Some(config))
}
