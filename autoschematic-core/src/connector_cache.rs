use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    // binary_cache::BinaryCache,
    connector::{
        Connector, ConnectorInbox, FilterOutput,
        parse::{connector_shortname, parse_connector_name},
        spawn::spawn_connector,
    },
    error::AutoschematicError,
    keystore::KeyStore,
};
use anyhow::Context;
use tokio::sync::Mutex;

// Connector name, Prefix
type HashKey = (String, PathBuf);
// Name, Prefix, Addr
// type FilterKey = (HashKey, PathBuf);

#[derive(Default)]
pub struct ConnectorCache {
    cache: Mutex<HashMap<HashKey, (Arc<Box<dyn Connector>>, ConnectorInbox)>>,
    /// Used to cache the results of Connector::filter(addr), which are assumed to be
    /// static. Since filter() is the most common call, this can speed up workflows by
    /// avoiding calling out to the connectors so many times.
    filter_cache: Mutex<HashMap<HashKey, HashMap<PathBuf, FilterOutput>>>,
    // binary_cache: BinaryCache,
}

/// A ConnectorCache represents a handle to multiple Connector instances. The server, CLI, and LSP
/// implementations all use a ConnectorCache to initialize connectors on-demand.
impl ConnectorCache {
    pub async fn get_connector(&self, name: &str, prefix: &Path) -> Option<(Arc<Box<dyn Connector>>, ConnectorInbox)> {
        let cache = self.cache.lock().await;

        let key = (name.into(), prefix.into());
        if let Some((connector, inbox)) = cache.get(&key) {
            Some((connector.clone(), inbox.resubscribe()))
        } else {
            None
        }
    }

    pub async fn get_or_spawn_connector(
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
            // Hence the song and dance below with the Arc and resubscribe().
            // let (connector, inbox) = spawn_connector(&connector_type, prefix, env, &self.binary_cache, keystore)
            let (connector, inbox) = spawn_connector(&connector_type, prefix, env, keystore)
                .await
                .context("spawn_connector()")?;

            if let Err(e) = connector.init().await {
                tracing::error!("Failed to init connector {}: {:#?}", connector_shortname(name)?, e);
            };

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

    pub async fn init_connector(&self, name: &str, prefix: &Path) -> Option<anyhow::Result<()>> {
        let cache = self.cache.lock().await;

        let connector_key = (name.into(), prefix.into());

        if let Some((connector, _inbox)) = cache.get(&connector_key) {
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
    pub async fn filter(&self, name: &str, prefix: &Path, addr: &Path) -> anyhow::Result<FilterOutput> {
        let connector_key = (name.into(), prefix.into());
        // let filter_key = (connector_key.clone(), addr.into());

        let mut filter_cache = self.filter_cache.lock().await;

        // Get the filter cache for connector `name` at prefix `prefix`, or initialize it.
        let connector_filter_cache = { filter_cache.entry(connector_key.clone()).or_insert_with(HashMap::new) };

        if let Some(value) = connector_filter_cache.get(addr) {
            Ok(*value)
        } else if let Some((connector, _inbox)) = self.cache.lock().await.get(&connector_key) {
            let res = connector.filter(addr).await?;
            connector_filter_cache.insert(addr.into(), res);
            Ok(res)
        } else {
            Ok(FilterOutput::None)
        }
    }

    ///
    pub async fn clear_filter_cache(&self, name: &str, prefix: &Path) {
        let connector_key = (name.into(), prefix.into());

        let mut filter_cache = self.filter_cache.lock().await;

        filter_cache.remove(&connector_key);
    }

    /// Drop all entries in the connector and filter caches.
    /// This should in theory kill all connectors that encapsulate running processes
    /// by calling their Drop impl.
    pub async fn clear(&self) {
        *self.cache.lock().await = HashMap::new();
        *self.filter_cache.lock().await = HashMap::new();
    }
}
