use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    config::Spec,
    connector::{
        Connector, ConnectorInbox, FilterOutput,
        spawn::spawn_connector,
    },
    error::AutoschematicError,
    keystore::KeyStore,
};

use anyhow::Context;
use dashmap::DashMap;

type HashKey = (String, PathBuf);

#[derive(Default)]
pub struct ConnectorCache {
    // TODO when we run different connectors init() in parallel, we're fine. But we were to run the same init() in parallel,
    // we'd currently end up initializing two instances and only writing the second one under this scheme.
    // TODO: make this a map of <HashKey, RwLock<...>> or similar, and let consumers instead block on read() until the background init holding write() is finished!
    cache: Arc<DashMap<HashKey, (Arc<dyn Connector>, ConnectorInbox)>>,
    /// Used to cache the results of Connector::filter(addr), which are assumed to be
    /// static. Since filter() is the most common call, this can speed up workflows by
    /// avoiding calling out to the connectors so many times.
    filter_cache: Arc<DashMap<HashKey, HashMap<PathBuf, FilterOutput>>>,
    // binary_cache: BinaryCache,
}

/// A ConnectorCache represents a handle to multiple Connector instances. The server, CLI, and LSP
/// implementations all use a ConnectorCache to initialize connectors on-demand.
impl ConnectorCache {
    pub async fn get_connector(&self, name: &str, prefix: &Path) -> Option<(Arc<dyn Connector>, ConnectorInbox)> {
        let key = (name.into(), prefix.into());
        if let Some(entry) = self.cache.get(&key) {
            let (connector, inbox) = &*entry;
            Some((connector.clone(), inbox.resubscribe()))
        } else {
            None
        }
    }

    pub async fn get_or_spawn_connector(
        &self,
        name: &str,
        spec: &Spec,
        prefix: &Path,
        env: &HashMap<String, String>,
        keystore: Option<Arc<dyn KeyStore>>,
    ) -> Result<(Arc<dyn Connector>, ConnectorInbox), AutoschematicError> {
        let key = (name.into(), prefix.into());

        if !self.cache.contains_key(&key) {
            // let connector_type = parse_connector_name(name)?;
            // In order for the first process that invokes connector_init to receive the earliest messages from the inbox,
            //  we need to pass the original inbox, and not the resubscribed copy.
            // Hence the song and dance below with the Arc and resubscribe().
            // let (connector, inbox) = spawn_connector(&connector_type, prefix, env, &self.binary_cache, keystore)
            let (connector, inbox) = spawn_connector(name, spec, prefix, env, keystore)
                .await
                .context("spawn_connector()")?;

            if let Err(e) = connector.init().await {
                tracing::error!("In prefix {}: failed to init connector {}: {:#?}", prefix.display(), name, e);
            };

            let connector_arc = Arc::new(connector);

            self.cache.insert(key.clone(), (connector_arc.clone(), inbox.resubscribe()));

            Ok((connector_arc, inbox))
        } else {
            let Some(entry) = self.cache.get(&key) else {
                return Err(anyhow::anyhow!("Failed to get connector from cache: name {}, prefix {:?}", name, prefix).into());
            };
            let (connector, inbox) = &*entry;
            Ok((connector.clone(), inbox.resubscribe()))
        }
    }

    pub async fn init_connector(&self, name: &str, prefix: &Path) -> Option<anyhow::Result<()>> {
        let connector_key = (name.into(), prefix.into());

        if let Some(entry) = self.cache.get(&connector_key) {
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
    pub async fn filter(&self, name: &str, prefix: &Path, addr: &Path) -> anyhow::Result<FilterOutput> {
        let connector_key = (name.into(), prefix.into());
        // let filter_key = (connector_key.clone(), addr.into());

        // Get the filter cache for connector `name` at prefix `prefix`, or initialize it.
        let mut connector_filter_cache = { self.filter_cache.entry(connector_key.clone()).or_default() };

        if let Some(value) = connector_filter_cache.get(addr) {
            Ok(*value)
        } else if let Some(entry) = self.cache.get(&connector_key) {
            let (connector, _inbox) = &*entry;
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

        self.filter_cache.remove(&connector_key);
    }

    /// Drop all entries in the connector and filter caches.
    /// This should in theory kill all connectors that encapsulate running processes
    /// by calling their Drop impl.
    pub async fn clear(&self) {
        self.cache.clear();
        self.filter_cache.clear();
    }
}
