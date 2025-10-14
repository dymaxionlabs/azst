use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::{cp, ls, mb, rb, rm};

#[derive(Parser)]
#[command(name = "azst")]
#[command(about = "A Rust CLI tool that wraps Azure CLI for easier storage container management")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Copy files to/from Azure storage (like gsutil cp)
    Cp {
        /// Source path (local file or az://container/path)
        source: String,
        /// Destination path (local file or az://container/path)
        destination: String,
        /// Recursive copy for directories
        #[arg(short, long)]
        recursive: bool,
        /// Parallel uploads/downloads
        #[arg(short = 'j', long, default_value = "4")]
        parallel: u32,
    },
    /// List objects in Azure storage (like gsutil ls)
    Ls {
        /// Path to list (az://container/ or az://container/prefix)
        path: Option<String>,
        /// Show detailed information
        #[arg(short, long)]
        long: bool,
        /// Show file sizes in human readable format
        #[arg(short = 'H', long)]
        human_readable: bool,
        /// Recursive listing
        #[arg(short, long)]
        recursive: bool,
        /// Storage account name
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Remove objects from Azure storage (like gsutil rm)
    Rm {
        /// Path to remove (az://container/path)
        path: String,
        /// Recursive removal
        #[arg(short, long)]
        recursive: bool,
        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Make bucket/container (like gsutil mb)
    Mb {
        /// Container name to create (az://container-name)
        container: String,
        /// Storage account name
        #[arg(short, long)]
        account: Option<String>,
    },
    /// Remove bucket/container (like gsutil rb)
    Rb {
        /// Container name to remove (az://container-name)
        container: String,
        /// Force removal of non-empty container
        #[arg(short, long)]
        force: bool,
    },
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        match &self.command {
            Commands::Cp {
                source,
                destination,
                recursive,
                parallel,
            } => cp::execute(source, destination, *recursive, *parallel).await,
            Commands::Ls {
                path,
                long,
                human_readable,
                recursive,
                account,
            } => {
                ls::execute(
                    path.as_deref(),
                    *long,
                    *human_readable,
                    *recursive,
                    account.as_deref(),
                )
                .await
            }
            Commands::Rm {
                path,
                recursive,
                force,
            } => rm::execute(path, *recursive, *force).await,
            Commands::Mb { container, account } => mb::execute(container, account.as_deref()).await,
            Commands::Rb { container, force } => rb::execute(container, *force).await,
        }
    }
}
