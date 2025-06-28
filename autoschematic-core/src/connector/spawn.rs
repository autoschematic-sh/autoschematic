use std::{collections::HashMap, path::Path, sync::Arc};

use crate::{
    config::Spec,
    connector::{Connector, ConnectorInbox},
    keystore::KeyStore,
};
use anyhow::Context;
#[cfg(feature = "python")]
use python::PythonConnector;


#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "sandbox")]
pub mod sandbox;

#[cfg(not(feature = "sandbox"))]
pub mod unsandbox;

pub async fn spawn_connector(
    shortname: &str,
    spec: &Spec,
    prefix: &Path,
    env: &HashMap<String, String>,
    // binary_cache: &BinaryCache,
    keystore: Option<Arc<dyn KeyStore>>,
) -> Result<(Box<dyn Connector>, ConnectorInbox), anyhow::Error> {
    let (outbox, inbox) = tokio::sync::broadcast::channel(64);

    // let lockfile = load_lockfile().await?;
    // let Some(connector_type) = lockfile::resolve_lock_entry(&lockfile, connector_type, binary_cache).await? else {
    //     bail!("No such entry in lockfile: {:?}", connector_type)
    // };

    match spec {
        Spec::Binary { path, protocol } => match protocol {
            crate::config::Protocol::Tarpc => Ok((
                Box::new(
                    unsandbox::launch_server_binary(spec, shortname, prefix, env, outbox, keystore)
                        .await
                        .context("launch_server_binary()")?,
                ) as Box<dyn Connector>,
                inbox,
            )),
        },
        Spec::Cargo { protocol, .. } => match protocol {
            crate::config::Protocol::Tarpc => Ok((
                Box::new(
                    unsandbox::launch_server_binary(spec, shortname, prefix, env, outbox, keystore)
                        .await
                        .context("launch_server_binary()")?,
                ) as Box<dyn Connector>,
                inbox,
            )),
        },
        Spec::CargoLocal { protocol, .. } => match protocol {
            crate::config::Protocol::Tarpc => Ok((
                Box::new(
                    unsandbox::launch_server_binary(spec, shortname, prefix, env, outbox, keystore)
                        .await
                        .context("launch_server_binary()")?,
                ) as Box<dyn Connector>,
                inbox,
            )),
        },
    }

    // match &connector_type {
    //     // TODO we need to inject env vars for python connectors!
    //     #[cfg(feature = "python")]
    //     super::r#type::ConnectorType::Python(path_buf, class_name) => Ok((
    //         PythonConnector::new(&format!("{}:{}", path_buf.to_string_lossy(), class_name), prefix, outbox).await?,
    //         inbox,
    //     )),
    //     #[cfg(not(feature = "python"))]
    //     super::r#type::ConnectorType::Python(path_buf, class_name) => {
    //         bail!("Python support not enabled.");
    //     }
    //     super::r#type::ConnectorType::LockFile(path_buf, short_name) => {
    //         bail!("Lockfile entry resolved to a lockfile entry {:?}", connector_type);
    //     }
    //     #[cfg(not(feature = "sandbox"))]
    //     super::r#type::ConnectorType::BinaryTarpc(binary_path, short_name) => Ok((
    //         Box::new(
    //             unsandbox::launch_server_binary(binary_path, short_name, prefix, env, outbox, keystore)
    //                 .await
    //                 .context("launch_server_binary()")?,
    //         ) as Box<dyn Connector>,
    //         inbox,
    //     )),
    //     #[cfg(feature = "sandbox")]
    //     super::r#type::ConnectorType::BinaryTarpc(binary_path, short_name) => Ok((
    //         Box::new(
    //             sandbox::launch_server_binary_sandboxed(binary_path, &short_name, prefix, &env, outbox, keystore)
    //                 .await
    //                 .context("launch_server_binary_sandboxed()")?,
    //         ) as Box<dyn Connector>,
    //         inbox,
    //     )),
    // }
}
