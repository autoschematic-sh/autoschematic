use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use autoschematic_core::config_rbac::AutoschematicRbacConfig;
use autoschematic_core::util::RON;
use jsonwebtoken::Algorithm;
use jsonwebtoken::encode;

use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use octocrab::params::pulls::MergeMethod;
use secrecy::SecretBox;
use serde::Serialize;

use crate::credentials::octocrab_user_client;

pub async fn create_pull_request(
    owner: &str,
    repo: &str,
    title: &str,
    head: &str,
    base: &str,
    client: &octocrab::Octocrab,
) -> anyhow::Result<u64> {
    let res = client.pulls(owner, repo).create(title, head, base).send().await?;
    Ok(res.number)
}

pub async fn create_comment(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    issue_number: u64,
    comment: &str,
) -> Result<(), anyhow::Error> {
    client.issues(owner, repo).create_comment(issue_number, comment).await?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct GithubJwtClaims {
    iat: u64,
    exp: u64,
    iss: String,
    alg: String,
}

pub async fn merge_pr(
    client: &octocrab::Octocrab,
    owner: &str,
    repo: &str,
    issue_number: u64,
    sha: &str,
) -> anyhow::Result<()> {
    client
        .pulls(owner, repo)
        .merge(issue_number)
        .title("Merged by autoschematic")
        .message("Merged by autoschematic")
        .method(MergeMethod::Rebase)
        .sha(sha)
        .send()
        .await?;

    Ok(())
}

pub async fn get_rbac_config_for_repo(
    owner: &str,
    repo: &str,
    user_access_token: &str,
) -> anyhow::Result<Option<AutoschematicRbacConfig>> {
    let user_client = octocrab_user_client(user_access_token).await?;

    let repository = user_client.repos(owner, repo).get().await?;

    let Some(default_branch) = repository.default_branch else {
        return Ok(None);
    };

    let config_content = user_client
        .repos(owner, repo)
        .get_content()
        .path("autoschematic.rbac.ron")
        .r#ref(default_branch)
        .send()
        .await;

    let contents = config_content?.take_items();
    let c = &contents[0];
    let decoded_content = c.decoded_content().unwrap_or_default();

    let config: AutoschematicRbacConfig = RON.from_str(&decoded_content)?;

    Ok(Some(config))
}
