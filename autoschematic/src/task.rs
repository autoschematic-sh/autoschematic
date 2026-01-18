use std::{
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use autoschematic_core::{
    keystore::KeyStore,
    util::load_autoschematic_config,
    workflow::{self},
};
use crossterm::style::Stylize;
use tokio::time::Instant;

use crate::{CONNECTOR_CACHE, safety_lock::check_safety_lock};

pub async fn run_task(path: &Path, _commit: bool, arg: Option<String>) -> anyhow::Result<()> {
    check_safety_lock()?;

    let config = load_autoschematic_config()?;

    let keystore = None;

    let mut arg = arg.map(|s| s.into_bytes());

    let mut state = None;

    loop {
        let keystore = keystore.as_ref().map(|k: &Arc<dyn KeyStore>| k.clone());
        let Some(res) =
            workflow::task_exec::task_exec(&config, CONNECTOR_CACHE.clone(), keystore, &None, path, arg, state).await?
        else {
            println!("{}: Not a task for any connector: {}", " Error".dark_red(), path.display());
            return Ok(());
        };

        if let Some(friendly_message) = res.friendly_message {
            println!(" ⋇ {}", friendly_message);
        }

        arg = None;
        state = res.next_state;

        if let Some(delay_until) = res.delay_until {
            let now_secs = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            if delay_until > now_secs
                && let Some(rerun_at) = Instant::now().checked_add(Duration::from_secs(delay_until - now_secs)) {
                    tokio::time::sleep_until(rerun_at).await;
                }
        }

        if state.is_none() {
            break;
        }
    }

    println!("{}", " Success!".dark_green());

    Ok(())
}
