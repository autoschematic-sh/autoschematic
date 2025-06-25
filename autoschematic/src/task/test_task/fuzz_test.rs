use std::path::{Path, PathBuf};

use anyhow::Context;
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};

use autoschematic_core::{
    git_util::git_add,
    util::{RON, copy_dir_all},
};

use crate::{apply, task::test_task::TestTask};

#[derive(Clone, Serialize, Deserialize)]
struct FuzzConfig {
    states: Vec<String>,
    connector_filter: Option<String>,
}

impl TestTask {
    pub async fn run_fuzz_test(&self, path: &Path) -> anyhow::Result<()> {
        let fuzz_config: FuzzConfig = RON.from_str(&std::fs::read_to_string(path.join("fuzz_config.ron"))?)?;

        let connector_filter = fuzz_config.connector_filter.map(|c| format!("-c {}", c)).unwrap_or_default();
        let prefix_filter = format!("-p {}", self.prefix.to_string_lossy());

        let mut i = 0;
        let mut samples: [u128; 10] = [0; 10];
        loop {
            let rand_suffix: String = rand::rng().sample_iter(&Alphanumeric).take(20).map(char::from).collect();

            for state in &fuzz_config.states {
                if i == 10 {
                    let mut sum = 0;
                    for j in 0..10 {
                        sum += samples[j];
                    }

                    println!("Frequency: {}", 1.0 / ((sum / 10_000_000_000) as f64));
                    i = 0;
                }
                samples[i] = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
                i += 1;
                copy_dir_all(path.join(state), &self.prefix).context("copy dir all")?;

                // tokio::time::sleep(Duration::from_secs(1)).await;

                git_add(&PathBuf::from("."), &PathBuf::from("."))?;

                // git_commit_and_push(&repo_path, &branch_name, &self.token, &format!("Fuzz state {}", state))?;

                let prefix = None;
                let connector = None;
                let subpath = None;
                let ask_confirm = false;
                let skip_commit = true;

                apply::apply(prefix, connector, subpath, ask_confirm, skip_commit).await?;

                let message = format!("fuzz-test-{}", rand_suffix);
                // println!("{}", message);
                // git_commit(&PathBuf::from("."), "autoschematic-fuzz", "fuzz@autoschematic.sh", &message)?;
            }
        }
    }
}
