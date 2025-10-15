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
    #[command(long_about = "Concatenate object content to stdout (like gsutil cat)

Examples:
  # Output blob contents to stdout
  azst cat az://myaccount/mycontainer/file.txt

  # Output multiple blobs
  azst cat az://myaccount/mycontainer/file1.txt az://myaccount/mycontainer/file2.txt

  # Print header for each blob
  azst cat --header az://myaccount/mycontainer/*.txt

  # Output specific byte range (start-end)
  azst cat -r 0-1023 az://myaccount/mycontainer/file.bin

  # Output from byte 1024 to end
  azst cat -r 1024- az://myaccount/mycontainer/file.bin

  # Redirect to file
  azst cat az://myaccount/mycontainer/file.txt > local_file.txt

  # Pipe to other commands
  azst cat az://myaccount/mycontainer/data.csv | head -10")]
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
    #[command(long_about = "Copy files to/from Azure storage (like gsutil cp)

Uses AzCopy backend for blazing-fast parallel transfers. Supports local-to-Azure, 
Azure-to-local, and Azure-to-Azure (server-side) operations.

Examples:
  # Copy file to Azure
  azst cp /local/file.txt az://myaccount/mycontainer/

  # Copy file from Azure
  azst cp az://myaccount/mycontainer/file.txt /local/

  # Copy directory recursively
  azst cp -r /local/dir/ az://myaccount/mycontainer/prefix/

  # Azure-to-Azure copy (server-side, no download/upload)
  azst cp -r az://account1/container1/data/ az://account2/container2/backup/

  # Preview operations without executing (dry-run)
  azst cp -r --dry-run /local/dir/ az://myaccount/mycontainer/

  # Limit bandwidth usage (in megabits per second)
  azst cp -r --cap-mbps 100 /large/dataset/ az://myaccount/mycontainer/

  # Filter files by pattern (supports wildcards)
  azst cp -r --include-pattern '*.jpg;*.png' /photos/ az://myaccount/photos/

  # Create MD5 hashes during upload
  azst cp -r --put-md5 /important-data/ az://myaccount/backup/

  # Use larger block sizes for large files
  azst cp -r --block-size-mb 32 /big-videos/ az://myaccount/media/")]
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
    #[command(long_about = "Display disk usage statistics (like gsutil du)

Shows disk usage for Azure storage containers and paths, or local directories.

Examples:
  # Show disk usage for entire container
  azst du az://myaccount/mycontainer/

  # Show disk usage for specific prefix
  azst du az://myaccount/mycontainer/data/

  # Show sizes in human-readable format (KB, MB, GB)
  azst du -H az://myaccount/mycontainer/

  # Show only total size
  azst du -s az://myaccount/mycontainer/

  # Show detailed breakdown with grand total
  azst du -Hc az://myaccount/mycontainer/

  # Calculate usage for all containers in an account
  azst du az://myaccount/

  # Calculate usage for local directory
  azst du /local/path/

  # Summarize local directory
  azst du -s /local/path/")]
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
    #[command(long_about = "List objects in Azure storage (like gsutil ls)

Lists storage accounts, containers, or objects. Supports wildcards and recursive listing.

Examples:
  # List all storage accounts
  azst ls

  # List all containers in a storage account
  azst ls az://myaccount/

  # List objects in a container
  azst ls az://myaccount/mycontainer/

  # List with detailed information
  azst ls -l az://myaccount/mycontainer/

  # List with human-readable sizes
  azst ls -lH az://myaccount/mycontainer/

  # Recursive listing
  azst ls -r az://myaccount/mycontainer/prefix/

  # List with wildcards
  azst ls 'az://myaccount/mycontainer/*.txt'")]
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
    #[command(long_about = "Move files to/from Azure storage (like gsutil mv)

Moves files by copying to destination and deleting from source. Supports local-to-Azure,
Azure-to-local, and Azure-to-Azure operations.

Examples:
  # Move file to Azure
  azst mv /local/file.txt az://myaccount/mycontainer/

  # Move file from Azure
  azst mv az://myaccount/mycontainer/file.txt /local/

  # Move directory recursively
  azst mv -r /local/dir/ az://myaccount/mycontainer/prefix/

  # Force move without confirmation
  azst mv -rf /local/file.txt az://myaccount/mycontainer/

  # Move between Azure accounts
  azst mv -r az://account1/container1/data/ az://account2/container2/")]
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
    #[command(long_about = "Remove objects from Azure storage (like gsutil rm)

Removes blobs from Azure storage or local files. Use with caution, especially 
with recursive and force flags.

Examples:
  # Remove single object
  azst rm az://myaccount/mycontainer/file.txt

  # Remove all objects with prefix (recursive)
  azst rm -r az://myaccount/mycontainer/prefix/

  # Force removal without confirmation
  azst rm -rf az://myaccount/mycontainer/old-data/

  # Preview what would be removed (dry-run)
  azst rm -r --dry-run az://myaccount/mycontainer/temp/

  # Remove everything except important files
  azst rm -r --exclude-pattern '*.db;*.config' az://myaccount/temp-data/

  # Remove only specific file types
  azst rm -r --include-pattern '*.log;*.tmp' az://myaccount/mycontainer/")]
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
    #[command(long_about = "Sync directories to/from Azure storage (like rsync)

Synchronizes a source directory to a destination, copying only changed or new files.
Optionally deletes files in destination that don't exist in source.

Examples:
  # Sync local directory to Azure
  azst sync /local/website/ az://myaccount/www/

  # Sync from Azure to local
  azst sync az://myaccount/backup/ /local/restore/

  # Sync with delete (remove extra files in destination)
  azst sync --delete /local/docs/ az://myaccount/documents/

  # Preview sync operations without executing
  azst sync --dry-run /local/data/ az://myaccount/backup/

  # Sync only text files, excluding temporary ones
  azst sync --include-pattern '*.txt;*.md' --exclude-pattern '*~;*.tmp' \\
    /documents/ az://myaccount/docs/

  # Limit bandwidth and ensure data integrity
  azst sync --cap-mbps 50 --put-md5 /backups/ az://myaccount/backup/")]
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
