use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use crossterm::style::Stylize;
use regex::Regex;
use tokio::fs::create_dir_all;

pub fn colour_op_message(message: &str) -> String {
    let re = Regex::new(r"(Deleted|deleted|DELETED|Delete|delete|DELETE|Destroy|destroy|DESTROY|DROPPED|DROP)").unwrap();

    // let message = re.replace_all(message, |captures: &regex::Captures| match &captures[0] {
    let message = re.replace_all(message, |captures: &regex::Captures| {
        let s = &captures[0];
        s.red().bold().underline(crossterm::style::Color::DarkGrey).to_string()
    });

    let re = Regex::new(r"(Created|created|CREATED|Create|create|CREATE)").unwrap();

    re.replace_all(&message, |captures: &regex::Captures| {
        let s = &captures[0];
        s.green().bold().to_string()
    })
    .into()
}

/// Look for a fenced ```diff … ``` block in `message`.
/// If found, colourise each added/removed line
pub fn try_colour_op_message_diff(message: &str) -> Option<String> {
    // (?s) → dot matches new-lines
    // (?m) → ^ / $ are line anchors
    let diff_re = Regex::new(r"(?sm)```diff\n(.*?)\n```").unwrap();

    if !diff_re.is_match(message) {
        return None;
    }

    let out = diff_re
        .replace_all(message, |caps: &regex::Captures| {
            let diff_body = &caps[1]; // text between the fences
            diff_body
                .lines()
                .map(|line| {
                    if line.starts_with('+') {
                        line.grey()
                            .on(crossterm::style::Color::Rgb { r: 38, g: 102, b: 33 })
                            .to_string() // green background
                    } else if line.starts_with('-') {
                        line.grey()
                            .on(crossterm::style::Color::Rgb { r: 145, g: 34, b: 17 })
                            .to_string() // red background
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .to_string();

    Some(out)
}

pub async fn ensure_config_dir() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("autoschematic");

    if !dir.exists() {
        create_dir_all(&dir).await.ok()?;
    }

    Some(dir)
}

pub async fn cached_motd() -> Option<(SystemTime, String)> {
    let dir = ensure_config_dir().await?;

    let motd_file = dir.join("motd");

    if motd_file.is_file() {
        let mtime = motd_file.metadata().ok()?.modified().ok()?;

        let motd = tokio::fs::read_to_string(motd_file).await.ok()?;

        return Some((mtime, motd));
    }
    None
}

pub async fn write_motd(motd: &str) -> Option<()> {
    let dir = ensure_config_dir().await?;

    let motd_file = dir.join("motd");

    tokio::fs::write(motd_file, motd).await.ok()?;

    Some(())
}

pub async fn try_fetch_motd() -> Option<String> {
    if std::env::var("AUTOSCHEMATIC_NO_MOTD").is_ok() {
        return None;
    }

    let mut old_motd: Option<String> = None;

    if let Some((mtime, motd)) = cached_motd().await {
        if mtime.elapsed().ok()? < Duration::from_secs(5) {
            return None;
        }

        old_motd = Some(motd);
    }

    let res = reqwest::Client::builder()
        .build()
        .ok()?
        .get("https://backend.autoschematic.sh/api/motd")
        .header("autoschematic-version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .ok()?;

    let new_motd = res.json::<HashMap<String, String>>().await.ok()?.get("motd").cloned();

    if old_motd == new_motd {
        return None;
    }

    if let Some(ref motd) = new_motd {
        write_motd(motd).await;
    }

    new_motd
}
