use std::{collections::HashMap, sync::Arc};

use octocrab::Octocrab;
use once_cell::sync::Lazy;
use secrecy::SecretBox;
use tokio::sync::Mutex;

use crate::{changeset::ChangeSet, error::AutoschematicServerError};

type Owner = String;
type Repo = String;
type PullRequest = u64;
type HeadSha = String;
type HashKey = (Owner, Repo, PullRequest);

#[derive(Default)]
pub struct ChangeSetCache {
    cache: Mutex<HashMap<HashKey, (Arc<Mutex<ChangeSet>>, HeadSha)>>,
}

impl ChangeSetCache {
    pub async fn get_or_init(
        &self,
        client: Octocrab,
        token: SecretBox<str>,
        repository: &octocrab::models::Repository,
        pull_request: &octocrab::models::pulls::PullRequest,
        owner: String,
        repo: String,
    ) -> Result<Arc<Mutex<ChangeSet>>, AutoschematicServerError> {
        let head_sha = pull_request.head.sha.clone();

        let mut cache = self.cache.lock().await;

        let key = (owner.clone(), repo.clone(), pull_request.number);

        if let Some((cached_changeset, cached_head_sha)) = cache.get(&key) {
            if *cached_head_sha != head_sha {
                // Cached changeset is stale! Remove it!
                cache.remove(&key);
                cache.insert(
                    key.clone(),
                    (
                        Arc::new(Mutex::new(
                            ChangeSet::from_pull_request(client, token, repository, pull_request, owner.clone(), repo.clone())
                                .await?,
                        )),
                        head_sha.clone(),
                    ),
                );
            } else {
                return Ok(cached_changeset.clone());
            }
        } else {
            cache.insert(
                key.clone(),
                (
                    Arc::new(Mutex::new(
                        ChangeSet::from_pull_request(client, token, repository, pull_request, owner.clone(), repo.clone())
                            .await?,
                    )),
                    head_sha.clone(),
                ),
            );
        }

        let Some((cached_changeset, _cached_head_sha)) = cache.get(&key) else {
            return Err(anyhow::anyhow!(
                "Failed to get changeset from cache: owner {}, repo {}, head_sha {}",
                owner,
                repo,
                head_sha
            )
            .into());
        };

        Ok(cached_changeset.clone())
    }

    pub async fn remove(&self, owner: String, repo: String, head_sha: String) {
        let cache = self.cache.lock().await;
    }
}

pub static CHANGESET_CACHE: once_cell::sync::Lazy<ChangeSetCache> = Lazy::new(|| ChangeSetCache::default());
