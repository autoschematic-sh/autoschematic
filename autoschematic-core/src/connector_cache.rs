use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    config::{self, AutoschematicConfig},
    connector::{
        Connector, ConnectorInbox, FilterResponse,
        handle::{ConnectorHandle, ConnectorHandleStatus},
        spawn::spawn_connector,
    },
    connector_util::check_connector_host_version_match,
    error::AutoschematicError,
    keystore::KeyStore,
    util::parse_env_file,
};

use anyhow::Context;
use dashmap::DashMap;
use serde::Serialize;
use tokio::task::JoinSet;

#[derive(Debug, Clone, Serialize)]
pub enum InitStatus {
    Offline,
    Spawning,
    Initializing,
    Error(String),
    Running,
}

#[derive(Debug, Serialize)]
pub struct TopResponse {
    handle_status: ConnectorHandleStatus,
    init_status: InitStatus,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ConnectorCacheKey {
    pub prefix: PathBuf,
    pub shortname: String,
}

pub type ConnectorCacheValue = (Arc<dyn ConnectorHandle>, ConnectorInbox);

#[derive(Default)]
pub struct ConnectorCache {
    cache: Arc<DashMap<ConnectorCacheKey, ConnectorCacheValue>>,
    init_status: Arc<DashMap<ConnectorCacheKey, InitStatus>>,
    /// Used to cache the results of Connector::filter(addr), which are assumed to be
    /// static. Since filter() is the most common call, this can speed up workflows by
    /// avoiding calling out to the connectors so many times.
    filter_cache: Arc<DashMap<ConnectorCacheKey, HashMap<PathBuf, FilterResponse>>>,
    // TODO add doc_cache
    // binary_cache: BinaryCache,
}

/// A ConnectorCache represents a handle to multiple Connector instances. The server, CLI, and LSP
/// implementations all use a ConnectorCache to initialize connectors on-demand.
impl ConnectorCache {
    pub async fn top(&self) -> HashMap<ConnectorCacheKey, TopResponse> {
        let mut res = HashMap::new();

        // Note: This looks redundant, but avoids a lifetime issue with DashMap: Map not being general enough...
        let keys: Vec<ConnectorCacheKey> = self.cache.iter().map(|kv| kv.key().clone()).collect();

        for key in keys {
            if let Some(kv) = self.cache.get(&key) {
                let init_status = match self.init_status.get(&key) {
                    Some(init_status) => init_status.value().clone(),
                    None => InitStatus::Initializing,
                };

                res.insert(
                    key,
                    TopResponse {
                        handle_status: kv.0.status().await,
                        init_status,
                    },
                );
            }
        }

        res
    }

    pub async fn get_connector(&self, name: &str, prefix: &Path) -> Option<(Arc<dyn Connector>, ConnectorInbox)> {
        let key = ConnectorCacheKey {
            shortname: name.into(),
            prefix: prefix.into(),
        };

        if let Some(entry) = self.cache.get(&key) {
            let (connector, inbox) = &*entry;
            Some((connector.clone(), inbox.resubscribe()))
        } else {
            None
        }
    }

    pub async fn get_or_spawn_connector(
        &self,
        config: &AutoschematicConfig,
        prefix: &str,
        connector_def: &config::Connector,
        keystore: Option<Arc<dyn KeyStore>>,
        do_init: bool,
    ) -> Result<(Arc<dyn Connector>, ConnectorInbox), AutoschematicError> {
        let key = ConnectorCacheKey {
            shortname: connector_def.shortname.clone(),
            prefix: prefix.into(),
        };

        let Some(prefix_def) = config.prefixes.get(prefix) else {
            return Err(anyhow::anyhow!(format!("No such prefix {}", prefix)).into());
        };

        let spec = &connector_def.spec;

        let mut env = HashMap::new();

        if let Some(ref env_file) = prefix_def.env_file {
            for (k, v) in parse_env_file(&std::fs::read_to_string(env_file).context(format!("Reading env file {}", env_file))?)
            {
                env.insert(k, v);
            }
        }

        for (k, v) in &prefix_def.env {
            env.insert(k.into(), v.into());
        }

        if let Some(ref env_file) = connector_def.env_file {
            for (k, v) in parse_env_file(&std::fs::read_to_string(env_file).context(format!("Reading env file {}", env_file))?)
            {
                env.insert(k, v);
            }
        }

        for (k, v) in &connector_def.env {
            env.insert(k.into(), v.into());
        }

        // let init_lock = match self.init_lock.entry(key.clone()) {
        //     dashmap::Entry::Occupied(occupied_entry) => *occupied_entry.get().write().await,
        //     dashmap::Entry::Vacant(vacant_entry) => {
        //         let lock = RwLock::new(());
        //         vacant_entry.insert(lock);
        //         // *vacant_entry..write().await
        //     }
        // };

        match self.cache.entry(key.clone()) {
            dashmap::Entry::Occupied(occupied_entry) => {
                let (connector, inbox) = occupied_entry.get();

                let need_init = match self.init_status.entry(key.clone()) {
                    dashmap::Entry::Occupied(status_ref) => match status_ref.get() {
                        InitStatus::Offline => do_init,
                        InitStatus::Spawning => do_init,
                        InitStatus::Initializing => false,
                        InitStatus::Error(_) => do_init,
                        InitStatus::Running => false,
                    },
                    dashmap::Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(InitStatus::Offline);
                        do_init
                    }
                };

                // check_connector_host_version_match(&connector_def.shortname, connector).await?;

                if need_init {
                    self.init_status.insert(key.clone(), InitStatus::Initializing);
                    // TODO there's a subtle race condition here - does it affect real usage?
                    // The init status is set, but the connector is yet to be initialized.
                    if let Err(e) = connector.init().await {
                        tracing::error!(
                            "In prefix {}: failed to init connector {}: {:#?}",
                            prefix,
                            connector_def.shortname,
                            e
                        );

                        self.init_status.insert(key.clone(), InitStatus::Error(format!("{:#?}", e)));
                    } else {
                        self.init_status.insert(key.clone(), InitStatus::Running);
                    }
                }

                Ok((connector.clone(), inbox.resubscribe()))
            }
            dashmap::Entry::Vacant(vacant_entry) => {
                self.init_status.insert(key.clone(), InitStatus::Spawning);
                // let connector_type = parse_connector_name(name)?;
                // In order for the first process that invokes connector_init to receive the earliest messages from the inbox,
                //  we need to pass the original inbox, and not the resubscribed copy.
                // Hence the song and dance below with the Arc and resubscribe().
                // let (connector, inbox) = spawn_connector(&connector_type, prefix, env, &self.binary_cache, keystore)
                let (connector, inbox) =
                    spawn_connector(&connector_def.shortname, spec, &PathBuf::from(prefix), &env, keystore)
                        .await
                        .context("spawn_connector()")?;

                check_connector_host_version_match(&connector_def.shortname, &connector).await?;

                if do_init {
                    self.init_status.insert(key.clone(), InitStatus::Initializing);
                    if let Err(e) = connector.init().await {
                        tracing::error!(
                            "In prefix {}: failed to init connector {}: {:#?}",
                            prefix,
                            connector_def.shortname,
                            e
                        );
                        self.init_status.insert(key.clone(), InitStatus::Error(format!("{:#?}", e)));
                    } else {
                        self.init_status.insert(key.clone(), InitStatus::Running);
                    }
                }

                let connector_arc = Arc::new(connector);

                vacant_entry.insert((connector_arc.clone(), inbox.resubscribe()));

                Ok((connector_arc, inbox))
            }
        }

        // if !self.cache.contains_key(&key) {
        //     // let connector_type = parse_connector_name(name)?;
        //     // In order for the first process that invokes connector_init to receive the earliest messages from the inbox,
        //     //  we need to pass the original inbox, and not the resubscribed copy.
        //     // Hence the song and dance below with the Arc and resubscribe().
        //     // let (connector, inbox) = spawn_connector(&connector_type, prefix, env, &self.binary_cache, keystore)
        //     let (connector, inbox) = spawn_connector(&connector_def.shortname, spec, &PathBuf::from(prefix), &env, keystore)
        //         .await
        //         .context("spawn_connector()")?;

        //     if do_init {
        //         if let Err(e) = connector.init().await {
        //             tracing::error!(
        //                 "In prefix {}: failed to init connector {}: {:#?}",
        //                 prefix,
        //                 connector_def.shortname,
        //                 e
        //             );
        //         };
        //         self.init_status.insert(key.clone(), true);
        //     }

        //     let connector_arc = Arc::new(connector);

        //     self.cache.insert(key.clone(), (connector_arc.clone(), inbox.resubscribe()));

        //     Ok((connector_arc, inbox))
        // } else {
        //     let Some(entry) = self.cache.get(&key) else {
        //         return Err(anyhow::anyhow!(
        //             "Failed to get connector from cache: name {}, prefix {:?}",
        //             connector_def.shortname,
        //             prefix
        //         )
        //         .into());
        //     };
        //     let (connector, inbox) = &*entry;

        //     if do_init && self.init_status.get(&key).is_none() {
        //         if let Err(e) = connector.init().await {
        //             tracing::error!(
        //                 "In prefix {}: failed to init connector {}: {:#?}",
        //                 prefix,
        //                 connector_def.shortname,
        //                 e
        //             );
        //         };
        //         self.init_status.insert(key.clone(), true);
        //     }

        // Ok((connector.clone(), inbox.resubscribe()))
        // }
    }

    pub async fn init_connector(&self, name: &str, prefix: &Path) -> Option<anyhow::Result<()>> {
        let key = ConnectorCacheKey {
            shortname: name.into(),
            prefix: prefix.into(),
        };

        if let Some(entry) = self.cache.get(&key) {
            let (connector, _inbox) = &*entry;
            self.clear_filter_cache(name, prefix).await;
            Some(connector.init().await)
        } else {
            None
        }
    }

    /// Since Connector::filter() must be a static function by contract,
    /// we cache its results to avoid expensive RPC calls.
    /// Note that this does not initialize connectors if they aren't yet present.
    /// Also, note that calling init() on a connector will invalidate the cached filter data.
    pub async fn filter_cached(&self, name: &str, prefix: &Path, addr: &Path) -> anyhow::Result<FilterResponse> {
        let key = ConnectorCacheKey {
            shortname: name.into(),
            prefix: prefix.into(),
        };

        // Get the filter cache for connector `name` at prefix `prefix`, or initialize it.
        let mut connector_filter_cache = { self.filter_cache.entry(key.clone()).or_default() };

        if let Some(value) = connector_filter_cache.get(addr) {
            Ok(*value)
        } else if let Some(entry) = self.cache.get(&key) {
            let (connector, _inbox) = &*entry;
            let res = connector.filter(addr).await?;
            connector_filter_cache.insert(addr.into(), res);
            Ok(res)
        } else {
            Ok(FilterResponse::none())
        }
    }

    pub async fn filter_all_cached(
        &self,
        autoschematic_config: &AutoschematicConfig,
        addr: &Path,
    ) -> anyhow::Result<FilterResponse> {
        for (prefix_name, prefix_def) in &autoschematic_config.prefixes {
            for connector_def in &prefix_def.connectors {
                match self
                    .filter_cached(&connector_def.shortname, &PathBuf::from(prefix_name), addr)
                    .await?
                {
                    FilterResponse::None => continue,
                    resp => return Ok(resp),
                }
            }
        }
        Ok(FilterResponse::None)
    }

    pub async fn clear_filter_cache(&self, name: &str, prefix: &Path) {
        let key = ConnectorCacheKey {
            shortname: name.into(),
            prefix: prefix.into(),
        };

        self.filter_cache.remove(&key);
    }

    /// Drop all entries in the connector and filter caches.
    /// This should in theory kill all connectors that encapsulate running processes
    /// by calling their Drop impl.
    pub async fn clear(&self) {
        let keys: Vec<ConnectorCacheKey> = self.cache.iter().map(|kv| kv.key().clone()).collect();

        let mut joinset = JoinSet::new();

        for key in keys {
            if let Some(kv) = self.cache.get(&key) {
                let connector = kv.0.clone();
                joinset.spawn(async move { connector.kill().await });
            }
        }

        joinset.join_all().await;

        self.cache.clear();
        self.filter_cache.clear();
    }
}

// TODO we'll revisit this later...
// #[async_trait]
// impl Connector for Arc<ConnectorCache> {
//     async fn new(name: &str, prefix: &Path, outbox: ConnectorOutbox) -> Result<Arc<dyn Connector>, anyhow::Error>
//     where
//         Self: Sized,
//     {
//         bail!("ConnectorCache::new() is a stub!")
//         // let connector_cache = ConnectorCache::default();
//         // Ok(Arc::new(connector_cache))
//     }

//     async fn init(&self) -> anyhow::Result<()> {
//         let mut joinset: JoinSet<anyhow::Result<()>> = JoinSet::new();

//         for (prefix, prefix_def) in &self.config.prefixes {
//             for connector_def in &prefix_def.connectors {
//                 let cache = self.clone();
//                 let prefix = prefix.clone();
//                 let connector_def = connector_def.clone();

//                 joinset.spawn(async move {
//                     cache
//                         .get_or_spawn_connector(
//                             &connector_def.shortname,
//                             &connector_def.spec,
//                             &PathBuf::from(prefix),
//                             &connector_def.env,
//                             cache.keystore.clone(),
//                             true,
//                         )
//                         .await?;
//                     Ok(())
//                 });
//             }
//         }

//         while let Some(res) = joinset.join_next().await {}

//         Ok(())
//     }

//     async fn filter(&self, addr: &Path) -> Result<FilterResponse, anyhow::Error> {
//         let keys: Vec<ConnectorCacheKey> = self.cache.iter().map(|kv| kv.key().clone()).collect();

//         let mut joinset: JoinSet<anyhow::Result<FilterResponse>> = JoinSet::new();

//         for key in keys {
//             let cache = self.clone();
//             let cache_addr = addr.to_owned();
//             joinset.spawn(async move { cache.filter_cached(&key.shortname, &key.prefix, &cache_addr).await });
//         }

//         while let Some(res) = joinset.join_next().await {
//             match res?? {
//                 FilterResponse::None => continue,
//                 other => return Ok(other),
//             }
//         }

//         Ok(FilterResponse::None)
//     }
// }
