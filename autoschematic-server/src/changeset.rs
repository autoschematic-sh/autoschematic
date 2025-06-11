use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, Context};
use askama::Template;
use autoschematic_core::{
    config::AutoschematicConfig, connector::Connector, connector_cache::ConnectorCache
};

use futures_util::TryStreamExt;
use git2::{Cred, IndexAddOption, PushOptions, RemoteCallbacks, Repository};
use octocrab::{
    models::{
        pulls::MergeableState,
        repos::DiffEntryStatus,
        webhook_events::{EventInstallation, WebhookEvent},
    },
    Octocrab,
};
use secrecy::{ExposeSecret, SecretBox};
use tempdir::TempDir;
use tokio::{pin, sync::Mutex};
use autoschematic_core::report::PlanReportSetOld;

use crate::{
    changeset_cache::CHANGESET_CACHE,
    credentials,
    error::{AutoschematicServerError, AutoschematicServerErrorType},
    object::{sort_objects_by_apply_order, Object},
    template::{random_failure_emoji, MiscError},
};

pub mod apply;
pub mod import;
pub mod import_skeletons;
pub mod plan;
pub mod pull_state;
pub mod solver;
pub mod trace;
pub mod types;
pub mod util;

// A ChangeSet essentially represents a pull request.
// The ChangeSet represents the handle used to call from a pull
//  request the various functions that an user, or operator,
//  will invoke to manipulate infra.
pub struct ChangeSet {
    pub temp_dir: TempDir,
    pub owner: String,
    pub repo: String,
    pub base_sha: String,
    pub head_sha: String,
    pub base_ref: String,
    pub head_ref: String,
    token: SecretBox<str>,
    issue_number: u64,
    pub client: Octocrab,
    pub objects: Vec<Object>,
    connector_cache: ConnectorCache,
    pub last_plan: Option<PlanReportSetOld>,
}

impl ChangeSet {
    // Create a ChangeSet from a Github WebhookEvent
    pub async fn from_webhook(
        webhook_event: &WebhookEvent,
    ) -> Result<Arc<Mutex<Self>>, AutoschematicServerError> {
        let Some(ref repository) = webhook_event.repository else {
            return Err(anyhow!("Webhook has no repository").into());
        };
        let owner = if let Some(owner) = &repository.owner {
            owner.login.clone()
        } else {
            return Err(anyhow!("Repo has no owner? (Maybe it's an org?)").into());
        };
        let repo = repository.name.clone();
        let payload = &webhook_event.specific;

        match payload {
            octocrab::models::webhook_events::WebhookEventPayload::IssueComment(payload) => {
                if let Some(EventInstallation::Minimal(ref installation_id)) =
                    webhook_event.installation
                {
                    let (client, token) =
                        credentials::octocrab_installation_client(installation_id.id).await?;
                    let pull_request = client
                        .pulls(owner.clone(), repo.clone())
                        .get(payload.issue.number)
                        .await?;

                    let changeset = CHANGESET_CACHE
                        .get_or_init(client, token, repository, &pull_request, owner, repo)
                        .await?;

                    Ok(changeset)

                    // return Self::from_pull_request(client, token, repository, pull_request, owner_name, repo_name).await;
                } else {
                    Err(AutoschematicServerError {
                        kind: AutoschematicServerErrorType::NotInstalled,
                    })
                }
            }
            _ => Err(AutoschematicServerError {
                // TODO this error type is wrong
                kind: AutoschematicServerErrorType::NotInstalled,
            }),
        }
    }

    pub async fn from_pull_request(
        client: Octocrab,
        token: SecretBox<str>,
        repository: &octocrab::models::Repository,
        pull_request: &octocrab::models::pulls::PullRequest,
        owner: String,
        repo: String,
    ) -> Result<Self, AutoschematicServerError> {
        let base_sha = pull_request.base.sha.clone();
        let head_sha = pull_request.head.sha.clone();
        let issue_number = pull_request.number;

        let head_ref = pull_request.head.ref_field.clone();
        let base_ref = pull_request.base.ref_field.clone();
        let Some(ref default_branch) = repository.default_branch else {
            tracing::error!("Repo has no default branch?");
            return Err(anyhow!("The current repository has no default branch.").into());
        };

        if Some(format!("{}:{}", owner, default_branch)) != pull_request.base.label {
            tracing::error!(
                "Default branch: {}, base.label: {:?}",
                default_branch,
                pull_request.base.label
            );
            return Err(anyhow!(
                "The current pull request is against {:?}, not the repo-default branch of {}.",
                pull_request.base.label,
                default_branch
            )
            .into());
        }

        match pull_request.mergeable_state {
            Some(MergeableState::Unstable) => {}
            Some(MergeableState::Unknown) => {}
            Some(MergeableState::Clean) => {}
            _ => {
                // self.create_comment()
                client
                    .issues(owner.clone(), repo.clone())
                    .create_comment(
                        issue_number,
                        MiscError {
                            error_message: format!(
                                "Pull request is not mergeable! Got mergeable_state: {:?}",
                                pull_request.mergeable_state
                            ),
                            failure_emoji: random_failure_emoji(),
                        }
                        .render()?,
                    )
                    .await?;
                // tracing::error!("Mergeable state: {:#?}", pull_request.mergeable_state);
                return Err(AutoschematicServerError {
                    kind: AutoschematicServerErrorType::NotMergeable,
                });
            }
        }
        // if mergeable_state != Some(MergeableState::Clean) {
        //     // TODO bail here! Unclean!
        //     tracing::error!("Mergeable state: {:#?}", mergeable_state);
        //     return Err(AutoschematicError {
        //         kind: AutoschematicErrorType::NotInstalled,
        //     })
        // }

        let stream_client = client.clone();
        let stream = client
            .pulls(&owner, &repo)
            .list_files(issue_number)
            .await?
            .into_stream(&stream_client);

        pin!(stream);

        let mut objects = Vec::new();
        while let Some(diff) = stream.try_next().await? {
            let path = PathBuf::from(diff.filename.clone());
            tracing::warn!(
                "Object {:?} : DiffEntryStatus: {:?} : prev = {:?}",
                path,
                diff.status,
                diff.previous_filename
            );

            // It seems like github is pretty lenient in what counts as a "rename", even with
            // minor edits. We count all renames as a remove and create, for now.
            // if diff.status == DiffEntryStatus::Renamed {
            if let Some(previous_filename) = diff.previous_filename {
                objects.push(Object {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    head_sha: head_sha.clone(),
                    filename: previous_filename.into(),
                    diff_status: DiffEntryStatus::Removed,
                });
            }
            // }
            objects.push(Object {
                owner: owner.clone(),
                repo: repo.clone(),
                head_sha: head_sha.clone(),
                filename: path,
                diff_status: diff.status,
            });
        }

        let temp_dir = TempDir::new("autoschematic")?;

        tracing::warn!(
            "unsorted objs: {:#?}",
            &objects
                .iter()
                .map(|a| (
                    a.filename.to_str().unwrap_or_default().into(),
                    a.diff_status.clone()
                ))
                .collect::<Vec<(String, DiffEntryStatus)>>()
        );

        let objects = sort_objects_by_apply_order(&objects);

        tracing::warn!(
            "sorted objs: {:#?}",
            &objects
                .iter()
                .map(|a| (
                    a.filename.to_str().unwrap_or_default().into(),
                    a.diff_status.clone()
                ))
                .collect::<Vec<(String, DiffEntryStatus)>>()
        );

        Ok(Self {
            temp_dir,
            owner,
            repo,
            base_sha,
            head_sha,
            base_ref,
            head_ref,
            token,
            issue_number,
            client,
            objects,
            connector_cache: ConnectorCache::default(),
            last_plan: None,
        })
    }

    pub async fn autoschematic_config(&self) -> Result<AutoschematicConfig, anyhow::Error> {
        // TODO if we're going to put access controls in autoschematic.ron,
        //  they are instantly defeated by reading from HEAD.
        // Therefore, we should read from whatever the default branch of the repo is.
        let autoschematic_config_object = Object {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            head_sha: self.head_sha.clone(),
            filename: "autoschematic.ron".into(),
            diff_status: DiffEntryStatus::Unchanged,
        };

        let autoschematic_config_file: AutoschematicConfig = autoschematic_config_object
            .parse_ron(&self.client)
            .await
            .context("Parsing autoschematic.ron")?;

        Ok(autoschematic_config_file)
    }

    pub fn git_add(&self, repo: &Repository, path: &Path) -> anyhow::Result<()> {
        let mut index = repo.index()?;
        index.add_all([path], IndexAddOption::default(), None)?;
        index.write()?;
        Ok(())
    }

    pub fn git_commit_and_push(&self, repo: &Repository, message: &str) -> anyhow::Result<()> {
        let mut index = repo.index()?;
        let oid = index.write_tree()?;
        let parent_commit = repo.head()?.peel_to_commit()?;
        let tree = repo.find_tree(oid)?;
        let sig = git2::Signature::now("autoschematic", "apply@autoschematic.sh")?;
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])?;

        let mut remote = repo.find_remote("origin")?;

        let refspec = format!("refs/heads/{}:refs/heads/{}", self.head_ref, self.head_ref);

        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
            // Typically, GitHub expects:
            //   - Username: "x-access-token"
            //   - Password: "<YOUR_TOKEN>"
            Cred::userpass_plaintext("x-access-token", self.token.expose_secret())
        });

        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);
        remote.push::<&str>(&[&refspec], Some(&mut push_options))?;
        Ok(())
    }

    // Write and commit the ./{prefix}/{addr}.out.json file,
    //  and push it to the PR.
    // This takes place after op_exec produces some outputs,
    //  such as a newly created EC2 instance's ID,
    //  or after get() imports a resource that similarly has outputs to store.
    // pub fn write_commit_output_file(
    //     &self,
    //     repo: &Repository,
    //     prefix: &Path,
    //     addr: &Path,
    //     phy_addr: Option<&Path>,
    //     outputs: &OutputMap,
    //     username: &str,
    //     comment_url: &str,
    //     merge_with_existing: bool,
    // ) -> Result<(), anyhow::Error> {
    //     if let Some(phy_addr) = phy_addr {
    //         let virt_output_path = build_out_path(prefix, addr);
    //         let phy_output_path = build_out_path(prefix, addr);

    //         // self.write_output_file(prefix, addr, Some(phy_addr), outputs, merge_with_existing)?;

    //         let mut index = repo.index()?;
    //         index.add_all(
    //             [virt_output_path, phy_output_path],
    //             IndexAddOption::default(),
    //             None,
    //         )?;
    //         index.write()?;
    //     } else {
    //         let output_path = build_out_path(prefix, addr);

    //         // self.write_output_file(prefix, addr, None, outputs, merge_with_existing)?;

    //         let mut index = repo.index()?;
    //         index.add_all([output_path], IndexAddOption::default(), None)?;
    //         index.write()?;
    //     }

    //     // TODO sign commits with a private key
    //     // repo.commit_signed(u, signature, None);

    //     let mut index = repo.index()?;
    //     let oid = index.write_tree()?;
    //     let parent_commit = repo.head()?.peel_to_commit()?;
    //     let tree = repo.find_tree(oid)?;
    //     let sig = git2::Signature::now("autoschematic", "apply@autoschematic.sh")?;
    //     let message = format!("autoschematic apply by @{}: {}", username, comment_url);
    //     repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&parent_commit])?;

    //     let mut remote = repo.find_remote("origin")?;

    //     let refspec = format!("refs/heads/{}:refs/heads/{}", self.head_ref, self.head_ref);
    //     remote.push::<&str>(&[&refspec], None)?;

    //     Ok(())
    // }
}
