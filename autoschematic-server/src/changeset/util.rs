use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::bail;
use autoschematic_core::{
    config_rbac::{self, AutoschematicRbacConfig},
    util::RON,
};
use git2::{
    Cred, FetchOptions, Oid, RemoteCallbacks, Repository,
    build::{CheckoutBuilder, RepoBuilder},
};
use octocrab::{
    models::{
        CheckRunId, CommentId,
        pulls::ReviewState,
        reactions::ReactionContent,
        webhook_events::{EventInstallation, WebhookEvent},
    },
    params::checks::{CheckRunConclusion, CheckRunStatus},
};
use secrecy::ExposeSecret;

use crate::{DOMAIN, chwd::ChangeWorkingDirectory, credentials, tracestore::TraceHandle};

use super::ChangeSet;

impl ChangeSet {
    /// Create a check run on the PR represented by this ChangeSet.
    pub async fn create_check_run(
        &self,
        check_run_id: Option<CheckRunId>,
        name: &str,
        url: &str,
        status: CheckRunStatus,
        conclusion: Option<CheckRunConclusion>,
    ) -> Result<CheckRunId, anyhow::Error> {
        Ok(match (check_run_id, conclusion) {
            (None, None) => {
                self.client
                    .checks(&self.owner, &self.repo)
                    .create_check_run(name, &self.head_sha)
                    .details_url(url)
                    .external_id(name)
                    .status(status)
                    .send()
                    .await?
                    .id
            }
            (None, Some(conclusion)) => {
                self.client
                    .checks(&self.owner, &self.repo)
                    .create_check_run(name, &self.head_sha)
                    .details_url(url)
                    .external_id(name)
                    .conclusion(conclusion)
                    .status(status)
                    .send()
                    .await?
                    .id
            }
            (Some(check_run_id), None) => {
                self.client
                    .checks(&self.owner, &self.repo)
                    .update_check_run(check_run_id)
                    .status(status)
                    .details_url(url)
                    .send()
                    .await?
                    .id
            }
            (Some(check_run_id), Some(conclusion)) => {
                self.client
                    .checks(&self.owner, &self.repo)
                    .update_check_run(check_run_id)
                    .conclusion(conclusion)
                    .status(status)
                    .details_url(url)
                    .send()
                    .await?
                    .id
            }
        })
    }

    pub async fn create_comment(&self, comment: &str) -> Result<(), anyhow::Error> {
        self.client
            .issues(self.owner.clone(), self.repo.clone())
            .create_comment(self.issue_number, comment)
            .await?;
        Ok(())
    }

    pub async fn add_reaction(&self, comment_id: CommentId, content: ReactionContent) -> Result<(), anyhow::Error> {
        self.client
            .issues(self.owner.clone(), self.repo.clone())
            .create_comment_reaction(comment_id, content)
            .await?;
        Ok(())
    }

    // Change working directory to the /tmp/... path in which we cloned the repo.
    // Reverts (pops) the working directory when chwd is dropped.
    pub fn chwd_to_repo(&self) -> Result<ChangeWorkingDirectory, anyhow::Error> {
        let repo_path = self.temp_dir.path().join(&self.owner).join(&self.repo);
        let chwd = ChangeWorkingDirectory::change(&repo_path)?;
        Ok(chwd)
    }

    pub fn repo_path(&self) -> PathBuf {
        self.temp_dir.path().join(&self.owner).join(&self.repo)
    }

    pub async fn clone_repo(&self) -> Result<Repository, anyhow::Error> {
        let owner_path = self.temp_dir.path().join(&self.owner);
        fs::create_dir_all(&owner_path)?;

        let repo_path = self.repo_path();

        let repo_url = format!("https://github.com/{}/{}.git", self.owner, self.repo);

        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
            // Typically, GitHub expects:
            //   - Username: "x-access-token"
            //   - Password: "<YOUR_TOKEN>"
            Cred::userpass_plaintext("x-access-token", self.token.expose_secret())
        });

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        fetch_opts.depth(1);
        let _checkout_opts = CheckoutBuilder::new();

        let repository = match Repository::open(&repo_path) {
            Ok(repository) => repository,
            Err(_) => RepoBuilder::new()
                .fetch_options(fetch_opts)
                .branch(&self.head_ref)
                .clone(&repo_url, &repo_path)?,
        };

        // TODO re-enable if we want to re-add git submodule support?
        // let submodules = repository.submodules()?;

        // let mut callbacks = RemoteCallbacks::new();
        // callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
        //     // Typically, GitHub expects:
        //     //   - Username: "x-access-token"
        //     //   - Password: "<YOUR_TOKEN>"
        //     Cred::userpass_plaintext("x-access-token", &self.token.expose_secret())
        // });

        // let mut fetch_opts = FetchOptions::new();
        // fetch_opts.remote_callbacks(callbacks);
        // fetch_opts.depth(1);
        // let _checkout_opts = CheckoutBuilder::new();

        // let mut submodule_update_options = SubmoduleUpdateOptions::new();
        // let update_opts = submodule_update_options.fetch(fetch_opts);

        // for mut submodule in submodules {
        //     tracing::info!(
        //         "Cloning submodule {:?} at {:?}",
        //         submodule.name(),
        //         submodule.path()
        //     );
        //     submodule.init(false)?;
        //     submodule.update(true, Some(update_opts))?;
        // }

        repository.remote_set_url("origin", &repo_url)?;

        repository.reset(
            &repository.find_object(Oid::from_str(&self.head_sha)?, None)?,
            git2::ResetType::Hard,
            None,
        )?;

        Ok(repository)
    }

    /// Get the RBAC config (autoschematic.rbac.ron) from the repository that this changeset points
    /// to. In particular, get the latest state of it as commited to the default branch (main, master, etc)
    /// as it would be useless to validate the config at the tip of a pull-request branch.
    pub async fn get_rbac_config(&self) -> anyhow::Result<Option<AutoschematicRbacConfig>> {
        let repository = self.client.repos(&self.owner, &self.repo).get().await?;

        let Some(default_branch) = repository.default_branch else {
            return Ok(None);
        };

        let config_content = self
            .client
            .repos(&self.owner, &self.repo)
            .get_content()
            .path("autoschematic.rbac.ron")
            .r#ref(default_branch)
            .send()
            .await;

        let contents = config_content?.take_items();
        if contents.is_empty() {
            return Ok(None);
        }

        let c = &contents[0];

        let Some(decoded_content) = c.decoded_content() else {
            return Ok(None);
        };

        let config: AutoschematicRbacConfig = RON.from_str(&decoded_content)?;

        Ok(Some(config))
    }

    pub async fn get_pr_approvals(&self) -> anyhow::Result<Vec<config_rbac::User>> {
        let pages = self
            .client
            .pulls(&self.owner, &self.repo)
            .list_reviews(self.issue_number)
            .per_page(100)
            .send()
            .await?;

        let mut reviews = self.client.all_pages(pages).await?;

        reviews.sort_by_key(|r| r.submitted_at.unwrap_or_default());

        let mut latest_reviews: HashMap<String, ReviewState> = HashMap::new();
        for r in reviews {
            if let (Some(user), Some(state)) = (r.user, r.state) {
                latest_reviews.insert(user.login, state);
            }
        }

        let mut approved = Vec::new();
        // let mut rejected = Vec::new();
        for (login, state) in latest_reviews {
            // match state {
            //     ReviewState::Approved => approved.push(config_rbac::User::GithubUser { username: login }),
            //     // ReviewState::ChangesRequested => rejected.push(login),
            //     _ => {} // COMMENTED / DISMISSED / PENDING don’t count
            // }

            if state == ReviewState::Approved {
                approved.push(config_rbac::User::GithubUser { username: login })
            }
        }

        Ok(approved)
    }
}

pub async fn create_comment_standalone(webhook_event: &WebhookEvent, comment: &str) -> anyhow::Result<()> {
    if let Some(EventInstallation::Minimal(ref installation_id)) = webhook_event.installation {
        let (client, _) = credentials::octocrab_installation_client(installation_id.id).await?;

        let Some(ref repository) = webhook_event.repository else {
            return Ok(());
        };

        let Some(ref owner) = repository.owner else {
            return Ok(());
        };

        let issue_number = match &webhook_event.specific {
            octocrab::models::webhook_events::WebhookEventPayload::IssueComment(payload) => payload.issue.number,
            octocrab::models::webhook_events::WebhookEventPayload::PullRequest(payload) => payload.number,
            _ => {
                bail!("create_comment_standalone: unhandled event type {:?}", webhook_event.specific)
            }
        };

        client
            .issues(owner.login.clone(), repository.name.clone())
            .create_comment(issue_number, comment)
            .await?;
    }
    Ok(())
}

#[allow(unused)]
pub async fn add_reaction_standalone(webhook_event: WebhookEvent, content: ReactionContent) -> anyhow::Result<()> {
    if let Some(EventInstallation::Minimal(installation_id)) = webhook_event.installation {
        let (client, _) = credentials::octocrab_installation_client(installation_id.id).await?;

        let Some(repository) = webhook_event.repository else {
            return Ok(());
        };

        let Some(owner) = repository.owner else {
            return Ok(());
        };

        let comment_id = match webhook_event.specific {
            octocrab::models::webhook_events::WebhookEventPayload::IssueComment(payload) => payload.comment.id,
            _ => {
                bail!("create_comment_standalone: unhandled event type {:?}", webhook_event.specific)
            }
        };

        client
            .issues(owner.login, repository.name)
            .create_comment_reaction(comment_id, content)
            .await?;
    }
    Ok(())
}

pub fn check_run_url(changeset: &ChangeSet, trace_handle: &TraceHandle) -> String {
    match DOMAIN.get() {
        Some(domain) => format!(
            "https://{}/dashboard/{}/{}/{}/#{}",
            domain, changeset.owner, changeset.repo, changeset.issue_number, trace_handle.run_key.run_id
        ),
        None => String::new(),
    }
}
