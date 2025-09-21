use crate::{
    config::AutoschematicConfig,
    connector::{FilterResponse, OutputMapFile, VirtToPhyResponse},
    connector_cache::ConnectorCache,
    keystore::KeyStore,
    util::{repo_root, split_prefix_addr},
};
use anyhow::{Context, bail};
use std::{path::Path, sync::Arc};

pub async fn rename(
    autoschematic_config: &AutoschematicConfig,
    connector_cache: &ConnectorCache,
    keystore: Option<Arc<dyn KeyStore>>,
    old_addr: &Path,
    new_addr: &Path,
) -> anyhow::Result<()> {
    // TODO chwd to the root of the git repo
    let _root = repo_root()?;

    let Some((old_prefix, old_virt_addr)) = split_prefix_addr(autoschematic_config, old_addr) else {
        bail!("Not in any prefix: {}", old_addr.display());
    };

    let Some((prefix, new_virt_addr)) = split_prefix_addr(autoschematic_config, new_addr) else {
        bail!("Not in any prefix: {}", new_addr.display());
    };

    if old_prefix != prefix {
        bail!("Can't modify prefix during a rename");
    }

    let Some(prefix_def) = autoschematic_config.prefixes.get(prefix.to_str().unwrap()) else {
        bail!("No such prefix: {}", prefix.display());
    };

    for connector_def in &prefix_def.connectors {
        // TODO Does rename, and therefore virt_to_phy/phy_to_virt require init()? Does virt to phy mapping
        // require the connector's config files, or ought it be statically determined by outputs alone?
        // (I'm leaning towards the latter!)
        let (connector, mut inbox) = connector_cache
            .get_or_spawn_connector(
                &connector_def.shortname,
                &connector_def.spec,
                &prefix,
                &connector_def.env,
                keystore.clone(),
                false,
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
            .filter_cached(&connector_def.shortname, &prefix, &old_virt_addr)
            .await?
            == FilterResponse::Resource
        {
            match connector.addr_virt_to_phy(&old_virt_addr).await? {
                VirtToPhyResponse::NotPresent => bail!("Phy address not present to rename"),
                VirtToPhyResponse::Deferred(_) => bail!("Phy address not present to rename"),
                VirtToPhyResponse::Null(_) => bail!("Rename: not a phy address"),
                VirtToPhyResponse::Present(phy_addr) => {
                    // let old_virt_output_path = build_out_path(&prefix, &old_virt_addr);
                    // let new_virt_output_path = build_out_path(&prefix, &new_virt_addr);

                    // if let Some(parent) = new_virt_output_path.parent() {
                    //     std::fs::create_dir_all(parent)?;
                    // }

                    let Some(output_map_file) = OutputMapFile::read_recurse(&prefix, &old_virt_addr)? else {
                        bail!(
                            "Rename: No output file found at {}/{}",
                            prefix.display(),
                            old_virt_addr.display()
                        )
                    };

                    OutputMapFile::write_link(&prefix, &phy_addr, &new_virt_addr)?;

                    output_map_file.write(&prefix, &new_virt_addr)?;

                    let new_virt_path = prefix.join(&new_virt_addr);

                    if let Some(parent) = new_virt_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    std::fs::copy(prefix.join(&old_virt_addr), prefix.join(&new_virt_addr)).context("copy virt")?;

                    std::fs::remove_file(prefix.join(&old_virt_addr))?;
                }
            }
        }
    }

    Ok(())
}
