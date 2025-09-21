use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, bail};
use tokio::task::JoinSet;

use crate::{
    bundle::{BundleMapFile, UnbundleResponseElement},
    config::AutoschematicConfig,
    connector::{Connector, FilterResponse},
    connector_cache::ConnectorCache,
    git_util::git_add,
    keystore::KeyStore,
    report::UnbundleReport,
    template::template_config,
    util::split_prefix_addr,
};

pub async fn write_unbundle_element(
    prefix: &Path,
    parent: &Path,
    element: &UnbundleResponseElement,
    overbundle: bool,
    git_stage: bool,
) -> anyhow::Result<()> {
    let output_path = prefix.join(element.addr.clone());

    if !overbundle && output_path.is_file() {
        if let Some(bundle_map) = BundleMapFile::read(prefix, &element.addr)? {
            match bundle_map {
                BundleMapFile::Bundle => {}
                BundleMapFile::ChildOf { parent: other_parent } => {
                    if parent != other_parent {
                        bail!(
                            "UnbundleReport::write_to_disk(): {} exists but belongs to a different bundle, and overbundle is not set.",
                            output_path.display()
                        )
                    }
                }
            }
        } else {
            bail!(
                "UnbundleReport::write_to_disk(): {} exists but is not in a bundle, and overbundle is not set.",
                output_path.display()
            )
        }
    }

    tokio::fs::write(&output_path, &element.contents).await?;
    if git_stage {
        git_add(&PathBuf::from("./"), &output_path)?;
    }

    let map_path = BundleMapFile::write_link(prefix, &element.addr, parent)?;
    if git_stage {
        git_add(&PathBuf::from("./"), &map_path)?;
    }

    Ok(())
}

pub async fn unbundle_connector(
    connector_shortname: &str,
    connector: Arc<dyn Connector>,
    prefix: &Path,
    virt_addr: &Path,
) -> Result<Option<UnbundleReport>, anyhow::Error> {
    let mut unbundle_report = UnbundleReport {
        prefix: prefix.into(),
        addr: virt_addr.into(),
        ..Default::default()
    };

    unbundle_report.elements = None;

    let path = prefix.join(virt_addr);

    if path.is_file() {
        let desired_bytes = tokio::fs::read(&path).await?;

        let elements = match std::str::from_utf8(&desired_bytes) {
            Ok(desired) => {
                let template_result = template_config(prefix, desired)?;

                if !template_result.missing.is_empty() {
                    for read_output in template_result.missing {
                        unbundle_report.missing_outputs.push(read_output);
                    }

                    return Ok(Some(unbundle_report));
                } else {
                    connector
                        .unbundle(virt_addr, template_result.body.as_bytes())
                        .await
                        .context(format!("{}::unbundle({}, _, _)", connector_shortname, virt_addr.display()))?
                }
            }
            Err(_) => connector.unbundle(virt_addr, &desired_bytes).await.context(format!(
                "{}::unbundle({}, _, _)",
                connector_shortname,
                virt_addr.display()
            ))?,
        };

        unbundle_report.elements = Some(elements);
    } else {
        return Ok(None);
    };

    Ok(Some(unbundle_report))
}

/// For a given path, attempt to resolve its prefix and Connector impl and return a Vec of UnbundleResponseElements.
/// `overbundle` decides whether to write a new bundle map file if a bundle should produce a file that exists, but is not marked as a child
/// of that bundle. If `overbundle` is false, only bundle result files that are already linked to that parent are written to disk.
pub async fn unbundle(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: Arc<ConnectorCache>,
    keystore: Option<Arc<dyn KeyStore>>,
    connector_filter: &Option<String>,
    path: &Path,
) -> Result<Option<UnbundleReport>, anyhow::Error> {
    let autoschematic_config = autoschematic_config.clone();

    let Some((prefix, virt_addr)) = split_prefix_addr(&autoschematic_config, path) else {
        return Ok(None);
    };

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix.to_str().unwrap_or_default()) else {
        return Ok(None);
    };

    let prefix_def = prefix_def.clone();

    let mut joinset: JoinSet<anyhow::Result<Option<UnbundleReport>>> = JoinSet::new();

    'connector: for connector_def in prefix_def.connectors {
        if let Some(connector_filter) = &connector_filter
            && connector_def.shortname != *connector_filter
        {
            continue 'connector;
        }

        let connector_cache = connector_cache.clone();
        let keystore = keystore.clone();
        let prefix = prefix.clone();
        let virt_addr = virt_addr.clone();
        joinset.spawn(async move {
            let (connector, mut inbox) = connector_cache
                .get_or_spawn_connector(
                    &connector_def.shortname,
                    &connector_def.spec,
                    &prefix,
                    &connector_def.env,
                    keystore,
                    true,
                )
                .await?;

            let _reader_handle = tokio::spawn(async move {
                loop {
                    match inbox.recv().await {
                        Ok(Some(stdout)) => {
                            eprintln!("{stdout}");
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                }
            });

            if connector_cache
                .filter_cached(&connector_def.shortname, &prefix, &virt_addr)
                .await?
                == FilterResponse::Bundle
            {
                let unbundle_report = unbundle_connector(&connector_def.shortname, connector, &prefix, &virt_addr).await?;
                return Ok(unbundle_report);
            }
            Ok(None)
        });
    }

    while let Some(res) = joinset.join_next().await {
        if let Some(unbundle_report) = res?? {
            return Ok(Some(unbundle_report));
        }
    }

    Ok(None)
}
