use std::path::PathBuf;

use clap::{Parser, Subcommand, command};
use sso::{login_via_github, persist_github_token};

mod config;
mod init;
mod install;
mod plan;
mod apply;
mod seal;
mod sso;
mod validate;
mod ui;

#[derive(Parser, Debug)]
#[command(name = "autoschematic")]
pub struct AutoschematicCommand {
    #[command(subcommand)]
    pub command: AutoschematicSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum AutoschematicSubcommand {
    /// Create an Autoschematic config if not already present.
    Init {},
    /// Validate that the Autoschematic config within this repository is well-formed.
    /// Includes autoschematic.lock.ron and autoschematic.rbac.ron if present.
    Validate {},
    /// Install a connector from a Github repository.
    Install {
        url: String,
        #[arg(short, long, default_value = None)]
        version: Option<String>,
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

        /// Prefix in which to create the sealed secret
        #[arg(long, default_value = None)]
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
    Login {
        /// Url of the Github organization to log in to, or github.com if omitted
        #[arg(long, default_value = None)]
        url: Option<String>,
    },
    /// Display the series of operations needed to apply the changeset.
    Plan {
        /// Optional path (can be a glob) to filter the changeset.
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
        /// Optional path (can be a glob) to filter the changeset.
        #[arg(short, long, value_name = "prefix")]
        prefix: Option<String>,

        /// Optional: run for a single connector by name
        #[arg(short, long, value_name = "connector")]
        connector: Option<String>,

        /// Optional path (can be a glob) to filter which resources are imported.
        #[arg(short, long, value_name = "subpath")]
        subpath: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = AutoschematicCommand::parse();

    match cmd.command {
        AutoschematicSubcommand::Seal {
            domain,
            prefix,
            path,
            in_path,
            key_id,
        } => {
            seal::seal(&domain, prefix.as_deref(), &path, in_path.as_deref(), key_id.as_deref()).await?;
        }
        AutoschematicSubcommand::Init {} => {
            init::init()?;
        }
        AutoschematicSubcommand::Validate {} => {
            validate::validate()?;
        }
        AutoschematicSubcommand::Login { url } => {
            let token = login_via_github().await?;
            persist_github_token(&token)?;
        }
        AutoschematicSubcommand::Install { url, version } => {
            install::install(&url, version).await?;
        }
        AutoschematicSubcommand::Plan {
            prefix,
            connector,
            subpath,
        } => {
            plan::plan(&prefix, &connector, &subpath).await?;
        }
        AutoschematicSubcommand::Apply {
            prefix,
            connector,
            subpath,
        } => {
            apply::apply(&prefix, &connector, &subpath).await?;
        }
    };

    Ok(())
}
