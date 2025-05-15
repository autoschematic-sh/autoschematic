use std::{fs, path::PathBuf};

use anyhow::bail;
use git2::{
    build::{CheckoutBuilder, RepoBuilder},
    Cred, FetchOptions, Oid, RemoteCallbacks, Repository,
};
use octocrab::{
    models::{
        reactions::ReactionContent,
        webhook_events::{EventInstallation, WebhookEvent},
        CheckRunId, CommentId,
    },
    params::checks::{CheckRunConclusion, CheckRunStatus},
};
use secrecy::ExposeSecret;

use crate::{
    chwd::ChangeWorkingDirectory, credentials, tracestore::TraceHandle, DOMAIN,
};

use super::ChangeSet;

impl ChangeSet {
    // Create a check run on the PR for this ChangeSet
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

    pub async fn add_reaction(
        &self,
        comment_id: CommentId,
        content: ReactionContent,
    ) -> Result<(), anyhow::Error> {
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
            Cred::userpass_plaintext("x-access-token", &self.token.expose_secret())
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

    // pub fn get_modified_objects(&self) -> anyhow::Result<Vec<PathBuf>> {
    //     let res = Vec::new();
    //     let repository = Repository::open(&self.repo_path())?;

    //     let base_obj = repository.revparse_single(&self.base_sha).context("revparse")?;
    //     let head_obj = repository.revparse_single(&self.head_sha).context("revparse")?;

    //     let base_tree = base_obj.peel(ObjectType::Tree).context("peel")?;
    //     let head_tree = head_obj.peel(ObjectType::Tree).context("peel")?;

    //     // let base_tree = repository.find_tree(Oid::from_str(&self.base_sha)?)?;
    //     // let head_tree = repository.find_tree(Oid::from_str(&self.head_sha)?)?;
        
    //     repository.revwalk()?;

    //     let diff = repository.diff_tree_to_tree(base_tree.as_tree(), head_tree.as_tree(), None)?;
    //     // let diff = repository.diff_blobs(Some(&base_tree), Some(&head_tree), None)?;

    //     // TODO figure out how to walk the revs
    //     //  to produce every file that has been touched between them!
    //     for delta in diff.deltas() {
    //         tracing::warn!(
    //             "Delta {:?}, {:?} -> {:?}",
    //             delta.status(),
    //             delta.old_file().path(),
    //             delta.new_file().path()
    //         );
    //     }

    //     Ok(res)
    // }
}

pub async fn create_comment_standalone(
    webhook_event: &WebhookEvent,
    comment: &str,
) -> anyhow::Result<()> {
    if let Some(EventInstallation::Minimal(ref installation_id)) = webhook_event.installation {
        let (client, _) = credentials::octocrab_installation_client(installation_id.id).await?;

        let Some(ref repository) = webhook_event.repository else {
            return Ok(());
        };

        let Some(ref owner) = repository.owner else {
            return Ok(());
        };

        let issue_number = match &webhook_event.specific {
            octocrab::models::webhook_events::WebhookEventPayload::IssueComment(payload) => {
                payload.issue.number
            }
            octocrab::models::webhook_events::WebhookEventPayload::PullRequest(payload) => {
                payload.number
            }
            _ => {
                bail!(
                    "create_comment_standalone: unhandled event type {:?}",
                    webhook_event.specific
                )
            }
        };

        client
            .issues(owner.login.clone(), repository.name.clone())
            .create_comment(issue_number, comment)
            .await?;
    }
    Ok(())
}

pub async fn add_reaction_standalone(
    webhook_event: WebhookEvent,
    content: ReactionContent,
) -> anyhow::Result<()> {
    if let Some(EventInstallation::Minimal(installation_id)) = webhook_event.installation {
        let (client, _) = credentials::octocrab_installation_client(installation_id.id).await?;

        let Some(repository) = webhook_event.repository else {
            return Ok(());
        };

        let Some(owner) = repository.owner else {
            return Ok(());
        };

        let comment_id = match webhook_event.specific {
            octocrab::models::webhook_events::WebhookEventPayload::IssueComment(payload) => {
                payload.comment.id
            }
            _ => {
                bail!(
                    "create_comment_standalone: unhandled event type {:?}",
                    webhook_event.specific
                )
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
            domain,
            changeset.owner,
            changeset.repo,
            changeset.issue_number,
            trace_handle.run_key.run_id
        ),
        None => String::new(),
    }
}

// pub file_exists_in_prefix(addr: &Path, prefix: &Path) {
// }
