// Download & cache release binaries from Github.
//

use std::{
    fs::{create_dir_all, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::bail;
use flate2::read::GzDecoder;
use flume::Receiver;
use futures::StreamExt;
use octocrab::models::repos::Asset;
use tar::Archive;

use crate::{flock::wait_for_flock, manifest::ConnectorManifest};

#[derive(Debug, Default)]
pub struct BinaryCache {
    // Binaries are downloaded and extracted to
    // {cache_folder}/{owner}/{repo}/{version}/
    cache_folder: PathBuf,
}

impl BinaryCache {
    pub fn new(cache_folder: &Path) -> anyhow::Result<Self> {
        create_dir_all(cache_folder)?;

        Ok(BinaryCache {
            cache_folder: cache_folder.to_path_buf(),
        })
    }

    pub async fn fetch_connector_release(
        &self,
        owner: &str,
        repo: &str,
        version: &str,
        manifest: &ConnectorManifest,
        arch: &str,
    ) -> anyhow::Result<PathBuf> {
        let out_dir = self
            .cache_folder
            .join(owner)
            .join(repo)
            .join(version);
            // .join(&asset_name);
        create_dir_all(&out_dir)?;

        let _filelock = wait_for_flock(out_dir.join(".lock")).await?;

        if out_dir.join(".clean").is_file() {
            tracing::info!("Reading {} from clean cache.", out_dir.to_string_lossy());
            return Ok(out_dir);
        }
        let release = octocrab::instance()
            .repos(owner, repo)
            .releases()
            .get_by_tag(version)
            .await?;

        // let mut manifest: Option<&Asset> = None;

        // for asset in &release.assets {
        //     if asset.name == "autoschematic.connector.ron" {
        //         connector_manifest = Some(asset);
        //         break;
        //     }
        // }

        // let Some(connector_manifest_asset) = connector_manifest else {
        //     bail!("No connector manifest found")
        // };

        // let mut asset_stream = octocrab::instance()
        //     .repos(owner, repo)
        //     .release_assets()
        //     .stream(*connector_manifest_asset.id)
        //     .await?;

        // let mut connector_manifest_s = Vec::new();
        // while let Some(item) = asset_stream.next().await {
        //     let mut chunk = item?.to_vec();
        //     connector_manifest_s.append(&mut chunk);
        // }
        // let connector_manifest: ConnectorManifest = RON.from_bytes(&connector_manifest_s)?;

        let asset_name = match manifest.r#type.as_str() {
            "binary-tarpc" => {
                format!("{}-{}.tar.gz", manifest.executable_name, arch)
            }
            _ => {
                format!("{}-noarch.tar.gz", manifest.executable_name)
            }
        };

        let mut connector_asset: Option<&Asset> = None;

        for asset in &release.assets {
            if asset.name == asset_name {
                connector_asset = Some(asset);
                break;
            }
        }

        let Some(connector_asset) = connector_asset else {
            bail!("No asset found under name {}", asset_name)
        };


        let mut asset_stream = octocrab::instance()
            .repos(owner, repo)
            .release_assets()
            .stream(*connector_asset.id)
            .await?;

        let (tx, rx) = flume::bounded(0);

        let decoder_thread = if asset_name.ends_with(".tar.gz") {
            let out_dir = out_dir.clone();
            std::thread::spawn(move || -> anyhow::Result<()> { Ok(gz_decode(&out_dir, rx)?) })
        } else {
            bail!(
                "Asset name {} not valid - Connector assets should be tar.gz archives.",
                asset_name
            );
        };

        while let Some(item) = asset_stream.next().await {
            let chunk = item?;
            tx.send_async(chunk.to_vec()).await?;
        }
        drop(tx); // close the channel to signal EOF

        let res = tokio::task::spawn_blocking(|| decoder_thread.join()).await?;
        match res {
            Ok(Ok(())) => {
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(out_dir.join(".clean"))?;
            }
            _ => {}
        }

        Ok(out_dir)
    }
}

fn gz_decode(out_path: &Path, rx: Receiver<Vec<u8>>) -> anyhow::Result<()> {
    let input = ChannelRead::new(rx);
    let gz = GzDecoder::new(input);
    let mut archive = Archive::new(gz);
    archive.unpack(out_path)?;
    Ok(())
}

fn copy_file(out_path: &Path, rx: Receiver<Vec<u8>>) -> anyhow::Result<()> {
    let mut input = ChannelRead::new(rx);
    let mut out_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .append(true)
        .open(out_path)?;
    std::io::copy(&mut input, &mut out_file)?;
    Ok(())
}

// Wrap a channel into something that impls `io::Read`
struct ChannelRead {
    rx: flume::Receiver<Vec<u8>>,
    current: std::io::Cursor<Vec<u8>>,
}

impl ChannelRead {
    fn new(rx: flume::Receiver<Vec<u8>>) -> ChannelRead {
        ChannelRead {
            rx,
            current: std::io::Cursor::new(vec![]),
        }
    }
}

impl Read for ChannelRead {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.current.position() == self.current.get_ref().len() as u64 {
            // We've exhausted the previous chunk, get a new one.
            if let Ok(vec) = self.rx.recv() {
                self.current = std::io::Cursor::new(vec);
            }
            // If recv() "fails", it means the sender closed its part of
            // the channel, which means EOF. Propagate EOF by allowing
            // a read from the exhausted cursor.
        }
        self.current.read(buf)
    }
}

mod tests {
    // use std::path::PathBuf;

    // use crate::binary_cache::BinaryCache;

    // #[tokio::test]
    // async fn test_download_gz() {
    //     rustls::crypto::ring::default_provider()
    //         .install_default()
    //         .expect("Failed to install rustls crypto provider");

    //     let cache_dir = PathBuf::from("/tmp/bincache");

    //     let cache = BinaryCache::new(&cache_dir);
    //     assert!(cache.is_ok());

    //     let cache = cache.unwrap();

    //     // TODO remove this before we start running tests very often and severely warping
    //     // download statistics for the IBM s390 build of this package...
    //     let res = cache
    //         .fetch_connector_release(
    //             "houseabsolute",
    //             "precious",
    //             "v0.7.3",
    //             "precious-Linux-s390x-gnu.tar.gz",
    //         )
    //         .await;
    //     assert!(res.is_ok());

    //     assert!(cache_dir
    //         .join("houseabsolute")
    //         .join("precious")
    //         .join("v0.7.3")
    //         .join("precious-Linux-s390x-gnu.tar.gz")
    //         .join("precious")
    //         .is_file());
    // }
}
