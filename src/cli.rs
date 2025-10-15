use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::commands::{cat, cp, du, ls, mv, rm, sync};

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
    /// Concatenate object content to stdout (like gsutil cat)
    Cat {
        /// URLs to read (az://container/path)
        urls: Vec<String>,
        /// Print short header for each object
        #[arg(long)]
        header: bool,
        /// Output just the specified byte range (e.g., '256-5939', '256-', or '-5')
        #[arg(short, long)]
        range: Option<String>,
    },
    /// Copy files to/from Azure storage (like gsutil cp)
    Cp {
        /// Source path (local file or az://container/path)
        source: String,
        /// Destination path (local file or az://container/path)
        destination: String,
        /// Recursive copy for directories
        #[arg(short, long)]
        recursive: bool,
        /// Preview what would be copied without actually copying
        #[arg(long)]
        dry_run: bool,
        /// Limit transfer rate in megabits per second
        #[arg(long)]
        cap_mbps: Option<f64>,
        /// Block size in MiB for upload/download (e.g., 8, 16, 32)
        #[arg(long)]
        block_size_mb: Option<f64>,
        /// Create MD5 hash for each file and save as Content-MD5 property
        #[arg(long)]
        put_md5: bool,
        /// Include only files matching this pattern (supports wildcards like *.jpg;*.pdf)
        #[arg(long)]
        include_pattern: Option<String>,
        /// Exclude files matching this pattern (supports wildcards like *.log;*.tmp)
        #[arg(long)]
        exclude_pattern: Option<String>,
    },
    /// Display disk usage statistics (like gsutil du)
    Du {
        /// Path to analyze (az://container/path or local path)
        path: Option<String>,
        /// Display only total size for each argument
        #[arg(short, long)]
        summarize: bool,
        /// Show sizes in human readable format (KB, MB, GB)
        #[arg(short = 'H', long)]
        human_readable: bool,
        /// Display grand total
        #[arg(short = 'c', long)]
        total: bool,
        /// Storage account name
        #[arg(short, long)]
        account: Option<String>,
    },
    /// List objects in Azure storage (like gsutil ls)
    Ls {
        /// Path to list (az://account/container/ or az://account/container/prefix)
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
    /// Move files to/from Azure storage (like gsutil mv)
    Mv {
        /// Source path (local file or az://container/path)
        source: String,
        /// Destination path (local file or az://container/path)
        destination: String,
        /// Recursive move for directories
        #[arg(short, long)]
        recursive: bool,
        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
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
        /// Preview what would be removed without actually removing
        #[arg(long)]
        dry_run: bool,
        /// Include only files matching this pattern (supports wildcards like *.jpg;*.pdf)
        #[arg(long)]
        include_pattern: Option<String>,
        /// Exclude files matching this pattern (supports wildcards like *.log;*.tmp)
        #[arg(long)]
        exclude_pattern: Option<String>,
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
        /// Preview what would be synced without actually syncing
        #[arg(long)]
        dry_run: bool,
        /// Limit transfer rate in megabits per second
        #[arg(long)]
        cap_mbps: Option<f64>,
        /// Block size in MiB for upload/download (e.g., 8, 16, 32)
        #[arg(long)]
        block_size_mb: Option<f64>,
        /// Create MD5 hash for each file and save as Content-MD5 property
        #[arg(long)]
        put_md5: bool,
        /// Include only files matching this pattern (supports wildcards like *.jpg;*.pdf)
        #[arg(long)]
        include_pattern: Option<String>,
        /// Exclude files matching this pattern (supports wildcards like *.log;*.tmp)
        #[arg(long)]
        exclude_pattern: Option<String>,
    },
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        match &self.command {
            Commands::Cat {
                urls,
                header,
                range,
            } => cat::execute(urls, *header, range.as_deref()).await,
            Commands::Cp {
                source,
                destination,
                recursive,
                dry_run,
                cap_mbps,
                block_size_mb,
                put_md5,
                include_pattern,
                exclude_pattern,
            } => {
                cp::execute(
                    source,
                    destination,
                    *recursive,
                    *dry_run,
                    *cap_mbps,
                    *block_size_mb,
                    *put_md5,
                    include_pattern.as_deref(),
                    exclude_pattern.as_deref(),
                )
                .await
            }
            Commands::Du {
                path,
                summarize,
                human_readable,
                total,
                account,
            } => {
                du::execute(
                    path.as_deref(),
                    *summarize,
                    *human_readable,
                    *total,
                    account.as_deref(),
                )
                .await
            }
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
            Commands::Mv {
                source,
                destination,
                recursive,
                force,
            } => mv::execute(source, destination, *recursive, *force).await,
            Commands::Rm {
                path,
                recursive,
                force,
                dry_run,
                include_pattern,
                exclude_pattern,
            } => {
                rm::execute(
                    path,
                    *recursive,
                    *force,
                    *dry_run,
                    include_pattern.as_deref(),
                    exclude_pattern.as_deref(),
                )
                .await
            }
            Commands::Sync {
                source,
                destination,
                delete,
                force,
                dry_run,
                cap_mbps,
                block_size_mb,
                put_md5,
                include_pattern,
                exclude_pattern,
            } => {
                sync::execute(
                    source,
                    destination,
                    *delete,
                    *force,
                    *dry_run,
                    *cap_mbps,
                    *block_size_mb,
                    *put_md5,
                    include_pattern.as_deref(),
                    exclude_pattern.as_deref(),
                )
                .await
            }
        }
    }
}
