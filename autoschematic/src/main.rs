use std::{path::PathBuf, sync::Arc};

use autoschematic_core::connector_cache::ConnectorCache;
use clap::{Parser, Subcommand};
use lazy_static::lazy_static;
use tracing_subscriber::EnvFilter;

use crate::safety_lock::{set_safety_lock, unset_safety_lock};

lazy_static! {
    /// Global connector cache shared across all subcommands
    pub static ref CONNECTOR_CACHE: Arc<ConnectorCache> = Arc::new(ConnectorCache::default());
}

mod apply;
mod config;
mod create;
mod import;
mod init;
mod install;
mod plan;
mod safety_lock;
mod seal;
mod spinner;
mod sso;
mod task;
mod unbundle;
mod util;
mod validate;

#[derive(Parser, Debug)]
#[command(name = "autoschematic")]
pub struct AutoschematicCommand {
    #[command(subcommand)]
    pub command: AutoschematicSubcommand,
}

#[derive(Subcommand, Default, Debug)]
pub enum AutoschematicInitSubcommand {
    #[default]
    Config,
    Rbac,
}

#[derive(Subcommand, Default, Debug)]
pub enum AutoschematicSafetySubcommand {
    #[default]
    Lock,
    Unlock,
}

#[derive(Subcommand, Debug)]
pub enum AutoschematicSubcommand {
    /// Create an Autoschematic config if not already present.
    Init {
        #[command(subcommand)]
        kind: Option<AutoschematicInitSubcommand>,
    },
    /// Set or unset the safety lock file. When set, the safety lock prevents any operations that would modify
    /// infrastructure (Executing ConnectorOps or Tasks).
    Safety {
        #[command(subcommand)]
        kind: AutoschematicSafetySubcommand,
    },
    /// Validate that the Autoschematic config within this repository is well-formed.
    /// Includes autoschematic.lock.ron and autoschematic.rbac.ron if present.
    Validate {},
    /// Install or upgrade the connectors listed in autoschematic.ron.
    Install {
        // url: String,
        // #[arg(short, long, default_value = None)]
        // version: Option<String>,
    },
    /// Seal a secret against a server's public key.
    Seal {
        /// Domain of the autoschematic server.
        /// autoschematic-seal will connect to the server to
        /// fetch one if its public keys. It will then use that public key
        /// with an ephemeral private key `epriv` to generate a shared symmetric encryption key,
        /// prompt the user to input a secret value,
        /// encrypt the secret value with the shared symmetric key,
        /// and write an output file to {prefix}/.secret/{path}.sealed .
        /// The output file will contain the ephemeral public key `epub`,
        /// the encrypted ciphertext `C`,
        /// and the signature of the concatenation of (`epub`, `C`) with `epriv`.
        #[arg(short, long)]
        domain: String,

        #[arg(long, default_value = None)]
        /// Prefix in which to create the sealed secret.
        prefix: Option<String>,

        #[arg(short, long)]
        /// Path of the sealed secret to create.
        /// Will create the secret at `./{prefix}/.secret/{path}.sealed`.
        path: PathBuf,

        #[arg(short, long)]
        /// Input file to read plaintext secret from.
        /// Prompts for hidden console input if not specified.
        in_path: Option<PathBuf>,

        #[arg(short, long, default_value = None)]
        /// Key ID from the server to encrypt the secret against.
        key_id: Option<String>,
    },
    // Login {
    //     /// Url of the Github organization to log in to, or github.com if omitted
    //     #[arg(long, default_value = None)]
    //     url: Option<String>,
    // },
    /// Display the series of operations needed to apply the changeset.
    Plan {
        /// Optional: run for a single prefix by name
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,
    },
    /// Execute the series of operations needed to apply the changeset.
    Apply {
        /// Optional: run for a single prefix by name
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,

        /// If set, don't ask for any confirmation before executing. Use with caution!
        #[arg(long, value_name = "skip_confirm", default_value_t = false)]
        skip_confirm: bool,

        /// If set, don't ask to run git commit (assume 'no').
        #[arg(long, value_name = "skip_commit", default_value_t = false)]
        skip_commit: bool,
    },
    /// Unpack bundle files to produce or refresh their children.
    Unbundle {
        /// Optional: run for a single prefix by name
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,

        /// If set, bundle outputs "clobber" existing files even if they weren't in the bundle before
        #[arg(long, value_name = "overbundle", default_value_t = false)]
        overbundle: bool,

        /// If set, don't stage the new bundle output files in git.
        #[arg(long, value_name = "no-stage", default_value_t = false)]
        no_stage: bool,
    },
    /// Execute a task as defined by a connector.
    RunTask {
        #[arg(short, long, value_name = "name")]
        name: String,
        #[arg(short, long, value_name = "prefix")]
        prefix: String,
    },
    /// Import remote resources into the repository.
    Import {
        /// Optional: run for a single prefix by name
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,

        /// If set, overwrite existing files with their remote state.
        #[arg(long, value_name = "overwrite", default_value_t = false)]
        overwrite: bool,

        /// If set, overwrite existing files with their remote state.
        #[arg(long, value_name = "commit")]
        commit: Option<bool>,
    },
    /// Scaffold new resource definitions from templates.
    Create {
        /// Optional path (can be a glob) to filter the changeset.
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(false)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Set up Ctrl-C handler to clean up connector cache
    let cache_for_ctrlc = CONNECTOR_CACHE.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("Received Ctrl-C, cleaning up connectors...");
            cache_for_ctrlc.clear().await;
            std::process::exit(130); // Standard exit code for SIGINT
        }
    });

    let cmd = AutoschematicCommand::parse();

    let result = match cmd.command {
        AutoschematicSubcommand::Seal {
            domain,
            path,
            in_path,
            key_id,
            ..
        } => seal::seal(&domain, &path, in_path.as_deref(), key_id.as_deref()).await,
        AutoschematicSubcommand::Init { kind } => match kind {
            None => init::init(),
            Some(AutoschematicInitSubcommand::Config) => init::init(),
            Some(AutoschematicInitSubcommand::Rbac) => init::init_rbac(),
        },
        AutoschematicSubcommand::Validate {} => validate::validate(),
        // AutoschematicSubcommand::Login { url } => {
        //     let token = login_via_github().await?;
        //     persist_github_token(&token)?;
        // }
        AutoschematicSubcommand::Install {} => install::install().await,
        AutoschematicSubcommand::Plan {
            prefix,
            connector,
            subpath,
        } => plan::plan(&prefix, &connector, &subpath).await,
        AutoschematicSubcommand::Apply {
            prefix,
            connector,
            subpath,
            skip_confirm,
            skip_commit,
        } => {
            let ask_confirm = !skip_confirm;
            apply::apply(prefix, connector, subpath, ask_confirm, skip_commit).await
        }
        AutoschematicSubcommand::Unbundle {
            prefix,
            connector,
            subpath,
            overbundle,
            no_stage,
        } => {
            let git_stage = !no_stage;
            unbundle::unbundle(&prefix, &connector, &subpath, overbundle, git_stage).await
        }
        AutoschematicSubcommand::Import {
            prefix,
            connector,
            subpath,
            overwrite,
            commit,
        } => import::import(prefix, connector, subpath, overwrite, commit).await,
        AutoschematicSubcommand::RunTask { name, prefix } => {
            task::spawn_task("", "", &PathBuf::from(prefix), &name, 0, serde_json::Value::Null, true).await
        }
        AutoschematicSubcommand::Create { prefix, connector } => create::create(&prefix, &connector).await,
        AutoschematicSubcommand::Safety { kind } => match kind {
            AutoschematicSafetySubcommand::Lock => set_safety_lock(),
            AutoschematicSafetySubcommand::Unlock => unset_safety_lock(),
        },
    };

    // Clean up connector cache before exiting
    CONNECTOR_CACHE.clear().await;

    result
}
