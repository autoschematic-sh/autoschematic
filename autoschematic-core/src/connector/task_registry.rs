use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use dashmap::DashMap;
use tokio::sync::RwLock;

use crate::{config::AutoschematicConfig, connector_cache::ConnectorCache};

#[derive(Default)]
pub struct ConnectorTaskRegistryEntry {
    pub addr: PathBuf,
    pub body: Vec<u8>,

    pub arg: Option<Vec<u8>>,
    pub state: Option<Vec<u8>>,

    pub modified_files: Option<Vec<PathBuf>>,
    pub outputs: Option<HashMap<String, Option<String>>>,
    pub secrets: Option<HashMap<PathBuf, Option<String>>>,
    pub friendly_messages: Option<Vec<String>>,
    pub delay_until: Option<u32>,
}

#[derive(Default)]
pub struct ConnectorTaskRegistry {
    connector_cache: ConnectorCache,
    pub entries: DashMap<String, RwLock<ConnectorTaskRegistryEntry>>,
}

impl ConnectorTaskRegistry {
    pub async fn init(&self) {}

    pub async fn start_task(
        config: &AutoschematicConfig,
        connector_cache: &ConnectorCache,
        prefix: &str,
        addr: &Path,
        arg: Vec<u8>,
    ) -> () {
        // TODO ok so here, we'll go through every connector in the config,
        // spawn or get it,
        // and then get which ever connector returns FilterResponse::Task.
        // Then, we'll spawn a tokio thread that will loop, 
        // and every iteration, it will run Connector::task_exec.
        // these tokio task handles will have to be stored in the registry too lol
        // kill on drop etc
        // 
    }
}
