use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    binary_cache::BinaryCache,
    connector::{parse::parse_connector_name, spawn::connector_init, Connector, ConnectorInbox},
    error::AutoschematicError,
    keystore::KeyStore,
};
use anyhow::Context;
use tokio::sync::Mutex;

// Name, Prefix
type HashKey = (String, PathBuf);
// Name, Prefix, Addr
type FilterKey = (HashKey, PathBuf);

#[derive(Default)]
pub struct ConnectorCache {
    cache: Mutex<HashMap<HashKey, (Arc<Box<dyn Connector>>, ConnectorInbox)>>,
    /// Used to cache the results of Connector::filter(addr), which are assumed to be
    /// static. Since filter() is the most common call, this can speed up workflows by
    /// avoiding calling out to the connectors so many times.
    filter_cache: Mutex<HashMap<FilterKey, bool>>,
    binary_cache: BinaryCache,
}

/// A ConnectorCache represents a handle to multiple Connector instances. The server, CLI, and LSP
/// implementations all use a ConnectorCache to initialize connectors on-demand.
impl ConnectorCache {
    pub async fn get(&self, name: &str, prefix: &Path) -> Option<(Arc<Box<dyn Connector>>, ConnectorInbox)> {
        let cache = self.cache.lock().await;

        let key = (name.into(), prefix.into());
        if let Some((connector, inbox)) = cache.get(&key) {
            Some((connector.clone(), inbox.resubscribe()))
        } else {
            None
        }
    }

    pub async fn get_or_init(
        &self,
        name: &str,
        prefix: &Path,
        env: &HashMap<String, String>,
        keystore: Option<&Box<dyn KeyStore>>,
    ) -> Result<(Arc<Box<dyn Connector>>, ConnectorInbox), AutoschematicError> {
        let mut cache = self.cache.lock().await;

        let key = (name.into(), prefix.into());

        if !cache.contains_key(&key) {
            let connector_type = parse_connector_name(name)?;
            // In order for the first process that invokes connector_init to receive the earliest messages from the inbox,
            //  we need to pass the original inbox, and not the resubscribed copy.
            // Hence the song and dance below.
            let (connector, inbox) = connector_init(&connector_type, prefix, env, &self.binary_cache, keystore)
                .await
                .context("connector_init()")?;

            let connector_arc = Arc::new(connector);

            cache.insert(key.clone(), (connector_arc.clone(), inbox.resubscribe()));

            Ok((connector_arc, inbox))
        } else {
            let Some((connector, inbox)) = cache.get(&key) else {
                return Err(anyhow::anyhow!("Failed to get connector from cache: name {}, prefix {:?}", name, prefix).into());
            };

            Ok((connector.clone(), inbox.resubscribe()))
        }
    }

    /// Since Connector::filter() must be a static function by contract,
    /// we cache its results to avoid expensive RPC calls.
    /// Note that this does not initialize connectors if they aren't yet present.
    pub async fn filter(&self, name: &str, prefix: &Path, addr: &Path) -> anyhow::Result<bool> {
        let connector_key = (name.into(), prefix.into());
        let filter_key = (connector_key.clone(), addr.into());

        let cached_lock_state = {
            let filter_cache = self.filter_cache.lock().await;
            filter_cache.get(&filter_key).cloned()
        };

        if let Some(value) = cached_lock_state {
            return Ok(value)
        } else if let Some((connector, _inbox)) = self.cache.lock().await.get(&connector_key) {
            let res = connector.filter(addr).await?;
            self.filter_cache.lock().await.insert(filter_key, res);
            Ok(res)
        } else {
            Ok(false)
        }
    }
    
    /// Drop all entries in the connector and filter caches.
    /// This should in theory kill all connectors that encapsulate running processes
    /// by calling their Drop impl.
    pub async fn clear(&self) {
        *self.cache.lock().await = HashMap::new();
        *self.filter_cache.lock().await = HashMap::new();
    }
}
