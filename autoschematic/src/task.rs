use std::{path::Path, sync::Arc};

use autoschematic_core::{
    keystore::KeyStore,
    util::load_autoschematic_config,
    workflow::{self},
};
use crossterm::style::Stylize;

use crate::CONNECTOR_CACHE;

pub async fn run_task(path: &Path, _commit: bool, arg: Option<String>) -> anyhow::Result<()> {
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

        arg = None;
        state = res.next_state;

        if state == None {
            break;
        }
    }

    println!("{}", " Success!".dark_green());

    Ok(())
}
