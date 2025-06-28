use std::{fs, path::PathBuf};

use anyhow::Context;
use autoschematic_core::{connector::parse::connector_shortname, glob::addr_matches_filter};
use git2::{Cred, PushOptions, RemoteCallbacks};
use secrecy::ExposeSecret;
use tokio::sync::broadcast::error::RecvError;

use crate::KEYSTORE;

use super::{
    ChangeSet,
    trace::{append_run_log, start_run},
};

impl ChangeSet {
    pub async fn import_skeletons(
        &self,
        subpath: Option<PathBuf>,
        connector_filter: Option<String>,
        comment_username: &str,
        comment_url: &str,
    ) -> Result<usize, anyhow::Error> {
        let trace_handle = start_run(self, comment_username, comment_url, "import", "").await?;

        let repo = self.clone_repo().await?;

        let autoschematic_config = self.autoschematic_config().await?;

        let subpath = subpath.unwrap_or(PathBuf::from("./"));

        // Number of skeletons found and imported
        let mut imported_count: usize = 0;

        for (prefix_name, prefix) in autoschematic_config.prefixes {
            for connector_def in prefix.connectors {
                let prefix_name = PathBuf::from(&prefix_name);
                let connector_shortname = connector_shortname(&connector_def.shortname)?;
                if let Some(connector_filter) = &connector_filter {
                    if connector_shortname != *connector_filter {
                        continue;
                    }
                }
                // temporarily cd into the repo...
                // I.E. `cd /tmp/autoschematic-298347928/pfnsec/autoschematic-playground`
                let _chwd = self.chwd_to_repo();

                let (connector, mut inbox) = self
                    .connector_cache
                    .get_or_spawn_connector(
                        &connector_def.shortname,
                        &connector_def.spec,
                        &prefix_name.clone(),
                        &connector_def.env,
                        Some(KEYSTORE.clone()),
                    )
                    .await?;
                let sender_trace_handle = trace_handle.clone();
                let _reader_handle = tokio::spawn(async move {
                    loop {
                        match inbox.recv().await {
                            Ok(Some(stdout)) => {
                                let res = append_run_log(&sender_trace_handle, stdout).await;
                                if let Ok(_) = res {}
                            }
                            Err(RecvError::Closed) => break,
                            _ => {}
                        }
                    }
                });

                let skeletons = connector
                    .get_skeletons()
                    .await
                    .context(format!("{}::get_skeletons()", connector_shortname))?;
                for skeleton in skeletons {
                    if !addr_matches_filter(&prefix_name.clone(), &skeleton.addr, &subpath) {
                        continue;
                    }

                    let addr = if skeleton.addr.is_absolute() {
                        skeleton.addr.strip_prefix("/")?
                    } else {
                        &skeleton.addr
                    };

                    let path = prefix_name.join(PathBuf::from(".skeletons").join(addr));
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    tokio::fs::write(&path, skeleton.body).await?;
                    self.git_add(&repo, &path)?;

                    imported_count += 1;
                }
            }
        }
        if imported_count > 0 {
            let mut index = repo.index()?;
            let oid = index.write_tree()?;
            let parent_commit = repo.head()?.peel_to_commit()?;
            let tree = repo.find_tree(oid)?;
            let sig = git2::Signature::now("autoschematic", "import@autoschematic.sh")?;
            let message = format!("autoschematic import-skeletons by @{}: {}", comment_username, comment_url);
            repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&parent_commit])?;

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
        }
        Ok(imported_count)
    }
}
