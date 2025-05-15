use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{
    binary_cache::{BinaryCache},
    connector::{Connector, ConnectorInbox},
    keystore::KeyStore,
    lockfile::{self, load_lockfile},
};
use anyhow::{bail, Context};
#[cfg(feature = "python")]
use python::PythonConnector;


use super::r#type::ConnectorType;

#[cfg(feature = "python")]
pub mod python;

pub mod sandbox;

pub async fn connector_init(
    connector_type: &ConnectorType,
    prefix: &Path,
    env: &HashMap<String, String>,
    binary_cache: &BinaryCache,
    keystore: Option<&Box<dyn KeyStore>>,
) -> Result<(Box<dyn Connector>, ConnectorInbox), anyhow::Error> {
    let (outbox, inbox) = tokio::sync::broadcast::channel(64);

    let lockfile = load_lockfile().await?;
    let Some(connector_type) =
        lockfile::resolve_lock_entry(&lockfile, connector_type, binary_cache).await?
    else {
        bail!("No such entry in lockfile: {:?}", connector_type)
    };

    match &connector_type {
        // TODO we need to inject env vars for python connectors!
        #[cfg(feature = "python")]
        super::r#type::ConnectorType::Python(path_buf, class_name) => Ok((
            PythonConnector::new(&format!("{}:{}", path_buf.to_string_lossy(), class_name), prefix, outbox).await?,
            inbox,
        )),
        #[cfg(not(feature = "python"))]
        super::r#type::ConnectorType::Python(path_buf, class_name) => {
            bail!("Python support not enabled.");
        }
        super::r#type::ConnectorType::LockFile(path_buf, short_name) => {
            bail!(
                "Lockfile entry resolved to a lockfile entry {:?}",
                connector_type
            );
        }
        super::r#type::ConnectorType::BinaryTarpc(binary_path, short_name) => Ok((
            Box::new(
                sandbox::launch_server_binary_sandboxed(
                    &PathBuf::from(binary_path),
                    &short_name,
                    prefix,
                    &env,
                    outbox,
                    keystore,
                )
                .await.context("launch_server_binary_sandboxed()")?,
            ) as Box<dyn Connector>,
            inbox,
        )),
    }
}
