use std::{collections::HashMap, time::Instant};

use anyhow::bail;
use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    Mutex,
};
use uuid::Uuid;

type Owner = String;
type Repo = String;
type PullRequest = u64;

#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct RepoKey {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct RunKey {
    pub pr: u64,
    pub run_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct RepoData {
    runs: IndexMap<RunKey, RunData>,
    run_sender: Sender<RunKey>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunData {
    #[serde(skip)]
    pub time_started: Instant,
    pub username: String,
    pub comment_url: String,
    pub r#type: String,
    pub command: String,
    pub logs: Vec<String>,
    #[serde(skip)]
    pub log_sender: Option<Sender<String>>,
    pub finished: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceHandle {
    pub repo_key: RepoKey,
    pub run_key: RunKey,
}

#[derive(Default, Debug)]
pub struct InMemTraceStore {
    repos: Mutex<HashMap<RepoKey, RepoData>>,
}

#[async_trait]
pub trait TraceStore: Send + Sync + std::fmt::Debug {
    async fn list_repos(&self) -> anyhow::Result<Vec<RepoKey>>;
    async fn list_runs(&self, repo_key: &RepoKey) -> anyhow::Result<Vec<RunKey>>;
    async fn list_runs_for_pr(&self, repo_key: &RepoKey, pr: u64) -> anyhow::Result<Vec<RunKey>>;
    async fn subscribe_runs(&self, repo_key: &RepoKey) -> anyhow::Result<Receiver<RunKey>>;
    async fn get_run(&self, repo_key: &RepoKey, run_key: &RunKey) -> anyhow::Result<RunData>;
    async fn delete_run(&self, repo_key: &RepoKey, run_key: &RunKey) -> anyhow::Result<()>;
    async fn put_run(
        &self,
        repo_key: &RepoKey,
        run_key: &RunKey,
        value: RunData,
    ) -> anyhow::Result<()>;
    async fn start_run(
        &self,
        owner: &str,
        repo: &str,
        pr: u64,
        username: &str,
        comment_url: &str,
        r#type: &str,
        command: &str,
    ) -> anyhow::Result<TraceHandle>;
    async fn finish_run(&self, handle: &TraceHandle) -> anyhow::Result<()>;
    async fn append_run_log(&self, handle: &TraceHandle, value: String) -> anyhow::Result<()>;
    async fn subscribe_run_logs(
        &self,
        repo_key: &RepoKey,
        run_key: &RunKey,
    ) -> anyhow::Result<Option<Receiver<String>>>;
}

#[async_trait]
impl TraceStore for InMemTraceStore {
    async fn list_repos(&self) -> anyhow::Result<Vec<RepoKey>> {
        let repos = self.repos.lock().await;

        let res: Vec<RepoKey> = repos.keys().map(|k| k.clone()).collect();
        Ok(res)
    }

    async fn list_runs(&self, key: &RepoKey) -> anyhow::Result<Vec<RunKey>> {
        let repos = self.repos.lock().await;
        let Some(repo) = repos.get(key) else {
            bail!("No such repo: {:?}", key);
        };

        let res: Vec<RunKey> = repo.runs.keys().map(|k| k.clone()).collect();
        Ok(res)
    }

    async fn list_runs_for_pr(&self, key: &RepoKey, pr: u64) -> anyhow::Result<Vec<RunKey>> {
        let repos = self.repos.lock().await;
        let Some(repo) = repos.get(key) else {
            bail!("No such repo: {:?}", key);
        };

        let res: Vec<RunKey> = repo
            .runs
            .keys()
            .filter(|k| k.pr == pr)
            .map(|k| k.clone())
            .collect();
        Ok(res)
    }

    async fn subscribe_runs(&self, repo_key: &RepoKey) -> anyhow::Result<Receiver<RunKey>> {
        let mut repos = self.repos.lock().await;
        let Some(repo) = repos.get_mut(repo_key) else {
            bail!("No such repo: {:?}", repo_key);
        };

        Ok(repo.run_sender.subscribe())
    }
    async fn get_run(&self, repo_key: &RepoKey, run_key: &RunKey) -> anyhow::Result<RunData> {
        let repos = self.repos.lock().await;
        let Some(repo) = repos.get(repo_key) else {
            bail!("No such repo: {:?}", repo_key);
        };

        let Some(run_data) = repo.runs.get(run_key) else {
            bail!("No such run: {:?}", run_key);
        };

        Ok(run_data.clone())
    }

    async fn delete_run(&self, repo_key: &RepoKey, run_key: &RunKey) -> anyhow::Result<()> {
        let mut repos = self.repos.lock().await;
        let Some(repo) = repos.get_mut(repo_key) else {
            bail!("No such repo: {:?}", repo_key);
        };

        repo.runs.remove(run_key);

        Ok(())
    }

    async fn put_run(
        &self,
        repo_key: &RepoKey,
        run_key: &RunKey,
        value: RunData,
    ) -> anyhow::Result<()> {
        let mut repos = self.repos.lock().await;

        if !repos.contains_key(repo_key) {
            let (run_sender, _) = tokio::sync::broadcast::channel(512);
            repos.insert(
                repo_key.clone(),
                RepoData {
                    runs: IndexMap::new(),
                    run_sender,
                },
            );
        }

        let Some(repo) = repos.get_mut(repo_key) else {
            bail!("No such repo: {:?}", repo_key);
        };

        repo.runs.insert(run_key.clone(), value);

        Ok(())
    }

    async fn start_run(
        &self,
        owner: &str,
        repo: &str,
        pr: u64,
        username: &str,
        comment_url: &str,
        r#type: &str,
        command: &str,
    ) -> anyhow::Result<TraceHandle> {
        let (log_sender, mut dummy_receiver) = tokio::sync::broadcast::channel(512);

        let run_data = RunData {
            time_started: Instant::now(),
            username: String::from(username),
            comment_url: String::from(comment_url),
            r#type: String::from(r#type),
            command: String::from(command),
            logs: Vec::new(),
            log_sender: Some(log_sender),
            finished: false,
        };
        
        
        let reader_handle = tokio::spawn(async move {
            loop {
                let res = dummy_receiver.recv().await;
                match res {
                    Ok(_) => {
                    }
                    Err(e) => {tracing::info!("dummy_receiver: {}", e);}
                }
            }
        });


        let repo_key = RepoKey {
            owner: String::from(owner),
            repo: String::from(repo),
        };

        let run_key = RunKey {
            pr: pr,
            run_id: Uuid::new_v4(),
        };
        self.put_run(&repo_key, &run_key, run_data).await?;

        return Ok(TraceHandle { repo_key, run_key });
    }

    async fn finish_run(&self, handle: &TraceHandle) -> anyhow::Result<()> {
        let mut repos = self.repos.lock().await;
        let Some(repo) = repos.get_mut(&handle.repo_key) else {
            bail!("No such repo: {:?}", handle.repo_key);
        };

        let Some(run_data) = repo.runs.get_mut(&handle.run_key) else {
            bail!("No such run: {:?}", &handle.run_key);
        };

        run_data.finished = true;

        Ok(())
    }

    async fn append_run_log(&self, handle: &TraceHandle, value: String) -> anyhow::Result<()> {
        let mut repos = self.repos.lock().await;

        let Some(repo) = repos.get_mut(&handle.repo_key) else {
            tracing::error!("No such repo: {:?}", handle.repo_key);
            bail!("No such repo: {:?}", handle.repo_key);
        };

        let Some(run_data) = repo.runs.get_mut(&handle.run_key) else {
            tracing::error!("No such run: {:?}", handle.run_key);
            bail!("No such run: {:?}", handle.run_key);
        };

        // if run_data.finished {
        //     tracing::error!("No such run: {:?}", handle.run_key);
        //     bail!("No such run: {:?}", handle.run_key);
        // }

        run_data.logs.push(value.clone());

        if let Some(sender) = &run_data.log_sender {
            match sender.send(value) {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("log_sender: {}", e);
                }
            }
        } else {
            tracing::info!("log_sender: None");
        }

        Ok(())
    }

    async fn subscribe_run_logs(
        &self,
        repo_key: &RepoKey,
        run_key: &RunKey,
    ) -> anyhow::Result<Option<Receiver<String>>> {
        let mut repos = self.repos.lock().await;
        let Some(repo) = repos.get_mut(repo_key) else {
            bail!("No such repo: {:?}", repo_key);
        };

        let Some(run_data) = repo.runs.get_mut(run_key) else {
            bail!("No such run: {:?}", run_key);
        };

        tracing::warn!("drop lock!");
        match &run_data.log_sender {
            Some(sender) => Ok(Some(sender.subscribe())),
            None => Ok(None),
        }
    }
}
