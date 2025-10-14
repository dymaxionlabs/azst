use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::{cp, ls, rm, sync};

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
    /// Sync directories to/from Azure storage (like rsync)
    Sync {
        /// Source path (local directory or az://container/path)
        source: String,
        /// Destination path (local directory or az://container/path)
        destination: String,
        /// Delete files in destination that don't exist in source
        #[arg(short, long)]
        delete: bool,
        /// Skip confirmation prompt for delete operations
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
            Commands::Sync {
                source,
                destination,
                delete,
                force,
            } => sync::execute(source, destination, *delete, *force).await,
        }
    }
}
