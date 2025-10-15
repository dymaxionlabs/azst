# azst - Azure Storage Tool

[![CI](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml)
[![Release](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

CLI tool for Azure Blob Storage with `gsutil`-like semantics. Uses **AzCopy** as
backend for blazing-fast transfers while providing a clean, intuitive interface.

## Features

- **🚀 High Performance** - Uses AzCopy backend for maximum transfer speeds
- **cat** - Concatenate object content to stdout (including byte range support)
- **cp** - Copy files to/from/between Azure storage (including Azure-to-Azure
  server-side copies)
- **ls** - List objects in Azure storage with detailed information
- **du** - Display disk usage statistics for storage containers and paths
- **rm** - Remove objects from Azure storage
- **Recursive operations** with `-r` flag
- **Human-readable file sizes** with `-h` flag
- **Parallel transfers** - Configurable with `-j` flag (default: 4 connections)
- **Clean URI format**: `az://<account>/<container>/path/to/object`
- **Familiar gsutil syntax** - Easy migration from Google Cloud Storage

## Why azst?

While AzCopy is powerful, it requires verbose HTTPS URLs. **azst** provides:
- **Cleaner syntax**: `az://account/container/path` instead of
  `https://account.blob.core.windows.net/container/path`
- **gsutil-like commands**: Familiar interface for GCP users
- **Better UX**: Intuitive error messages and colored output
- **AzCopy performance**: Fast parallel transfers under the hood

## Prerequisites

1. **Azure CLI**: Install from [https://docs.microsoft.com/en-us/cli/azure/install-azure-cli](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli)
2. **Authentication**: Run `az login` to authenticate with Azure

**Note**: AzCopy will be automatically downloaded and installed during the installation process.

## Installation

### Quick Install (Recommended)

Install the latest build using curl:

```bash
curl -sSL https://raw.githubusercontent.com/dymaxionlabs/azst/main/install.sh | bash
```

This will download and install:
- The latest `azst` binary from the `main` branch for your system to `~/.local/bin/`
- AzCopy version 10.30.1 to `~/.local/share/azst/azcopy/` (if not already present)

### Manual Installation

Download the latest build for your platform from the [releases page](https://github.com/dymaxionlabs/azst/releases/tag/latest).

#### macOS / Linux

```bash
# Download and extract
tar xzf azst-*.tar.gz

# Move to a directory in your PATH
sudo mv azst /usr/local/bin/

# Or install to user directory
mkdir -p ~/.local/bin
mv azst ~/.local/bin/
export PATH="$PATH:~/.local/bin"  # Add to your ~/.bashrc or ~/.zshrc
```

#### Windows

Download the `.zip` file, extract it, and add the directory to your PATH.

**Note**: Manual installation requires AzCopy to be installed separately from
[https://aka.ms/downloadazcopy](https://aka.ms/downloadazcopy), or you can run
the installation script to automatically download AzCopy v10.30.1.

### Build from Source

Requires [Rust](https://rustup.rs/) to be installed.

```bash
# Clone the repository
git clone https://github.com/dymaxionlabs/azst
cd azst

# Build and install
cargo install --path .
```

The binary will be installed to `~/.cargo/bin/azst` (make sure this directory is
in your PATH).

## Usage

### Basic Commands

```bash
# List all storage accounts (like gsutil ls)
azst ls

# List all containers in a storage account
azst ls az://myaccount/

# List objects in a container
azst ls az://myaccount/mycontainer/

# List with detailed information
azst ls -l az://myaccount/mycontainer/

# Copy file to Azure
azst cp /local/file.txt az://myaccount/mycontainer/

# Copy file from Azure
azst cp az://myaccount/mycontainer/file.txt /local/

# Copy directory recursively
azst cp -r /local/dir/ az://myaccount/mycontainer/prefix/

# Remove object
azst rm az://myaccount/mycontainer/file.txt

# Remove all objects with prefix (recursive)
azst rm -r az://myaccount/mycontainer/prefix/

# Display disk usage statistics
azst du az://myaccount/mycontainer/

# Display disk usage with human-readable sizes
azst du -H az://myaccount/mycontainer/prefix/

# Show only total size (summarize)
azst du -s az://myaccount/mycontainer/
```

### Concatenate (cat)

The `cat` command outputs the contents of one or more Azure blobs to stdout,
similar to `gsutil cat`:

```bash
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
azst cat az://myaccount/mycontainer/data.csv | head -10
```

**Options:**
- `--header`: Print short header for each object
- `-r, --range <RANGE>`: Output just the specified byte range
  - Format: `start-end` (e.g., `256-5939`)
  - Format: `start-` (e.g., `256-` for all bytes from 256 onwards)

**Note:** The `cat` command does not compute a checksum of the downloaded data.
For data integrity verification, use `azst cp` which performs automatic checksum
validation.

### Disk Usage (du)

The `du` command displays disk usage statistics for Azure storage or local paths,
similar to Linux `du` and `gsutil du`:

```bash
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
azst du -s /local/path/
```

**Options:**
- `-s, --summarize`: Display only the total size for each argument
- `-H, --human-readable`: Show sizes in human-readable format (KB, MB, GB)
- `-c, --total`: Display grand total at the end
- `-a, --account <ACCOUNT>`: Specify storage account name

**Output format:**
```
SIZE    PATH
1.5 GB  az://myaccount/mycontainer/data/
500 MB  az://myaccount/mycontainer/logs/
2.0 GB  total: az://myaccount/mycontainer/
```


### Advanced Usage

```bash
# Copy with parallel connections (default: 4, increase for better performance)
azst cp -r -j 16 /large/directory/ az://myaccount/mycontainer/

# Azure-to-Azure copy (server-side, no download/upload)
azst cp -r az://account1/container1/data/ az://account2/container2/backup/

# Force operations without confirmation
azst rm -rf az://myaccount/mycontainer/prefix/

# List with human-readable sizes
azst ls -lh az://myaccount/mycontainer/

# Preview operations without executing (dry-run)
azst cp -r --dry-run /local/dir/ az://myaccount/mycontainer/
azst rm -r --dry-run az://myaccount/mycontainer/old-files/

# Limit bandwidth usage (in megabits per second)
azst cp -r --cap-mbps 100 /large/dataset/ az://myaccount/mycontainer/

# Filter files by pattern (supports wildcards)
azst cp -r --include-pattern "*.jpg;*.png" /photos/ az://myaccount/photos/
azst rm -r --exclude-pattern "*.log;*.tmp" az://myaccount/mycontainer/

# Combine filtering options
azst sync --include-pattern "*.txt" --exclude-pattern "*temp*" \
  /local/docs/ az://myaccount/documents/
```

### Performance Notes

**azst uses AzCopy as the backend for copy operations**, which means:
- ✨ **Parallel transfers** by default (configurable with `-j` flag)
- 🚀 **Much faster** than uploading files one-by-one
- ⚡ **Azure-to-Azure copies** are server-side (no data transfer through your
  machine)
- 📊 **Optimized for large files** with block-level parallelism
- 🔄 **Automatic retries** and network optimization

**Comparison with other tools:**
- Similar performance to native AzCopy
- 2-10x faster than Azure CLI (`az storage`) for large transfers
- Comparable to `gsutil -m` for Google Cloud Storage

**When to use `-j` flag:**
- Default (4 connections): Good for most cases
- Higher values (8-16): Large directories with many small files
- Very high (32+): Only if you have excellent bandwidth and many files


### URI Format

Azure URIs follow the format:
`az://<storage-account>/<container>/path/to/object`

This convention is specific to `azst` and provides a self-contained way to
reference Azure storage resources:

- `azst ls` - List all storage accounts (similar to `gsutil ls`)
- `az://myaccount/` - List all containers in storage account
- `az://myaccount/mycontainer/` - List all objects in container
- `az://myaccount/mycontainer/prefix/` - List objects with prefix
- `az://myaccount/mycontainer/file.txt` - Specific object

**Legacy format** (without storage account) is also supported for backward
compatibility:
- `az://mycontainer/` - Requires `--account` flag or default configuration

**Note:** The `az://` URI scheme is not used by Microsoft Azure services, so
there are no conflicts with official tools.

## Advanced Options

### Dry Run

Preview what operations would be performed without actually executing them:

```bash
# Preview file copies
azst cp -r --dry-run /local/data/ az://myaccount/backup/

# Preview file removals
azst rm -r --dry-run az://myaccount/old-container/

# Preview sync operations
azst sync --dry-run /local/website/ az://myaccount/www/
```

### Bandwidth Control

Limit transfer rates to prevent saturating your network connection:

```bash
# Limit to 100 Mbps
azst cp -r --cap-mbps 100 /large/files/ az://myaccount/container/

# Useful for background uploads that shouldn't affect other network usage
azst sync --cap-mbps 50 /backups/ az://myaccount/backup-container/
```

### Performance Tuning

Optimize transfer performance with block size control:

```bash
# Use larger block sizes for large files (default: auto-calculated)
azst cp -r --block-size-mb 32 /big-videos/ az://myaccount/media/

# Use smaller block sizes for many small files
azst cp -r --block-size-mb 4 /logs/ az://myaccount/logs/

# Combine with bandwidth limiting for controlled uploads
azst sync --block-size-mb 16 --cap-mbps 100 /data/ az://myaccount/backup/
```

**Recommended block sizes:**
- Small files (< 10 MB): 4-8 MiB
- Medium files (10-100 MB): 8-16 MiB
- Large files (> 100 MB): 16-32 MiB
- Very large files (> 1 GB): 32-100 MiB

### Data Integrity

Enable MD5 hashing to ensure data integrity:

```bash
# Create MD5 hashes during upload
azst cp -r --put-md5 /important-data/ az://myaccount/backup/

# Useful for critical data that requires verification
azst sync --put-md5 /production-db/ az://myaccount/db-backup/
```

**Note:** MD5 hashing adds some overhead but ensures data integrity. The hash is
saved as the blob's Content-MD5 property.

### Pattern Filtering

Filter files using wildcards during copy, sync, or remove operations:

```bash
# Copy only image files
azst cp -r --include-pattern "*.jpg;*.png;*.gif" /photos/ az://myaccount/images/

# Remove everything except important files
azst rm -r --exclude-pattern "*.db;*.config" az://myaccount/temp-data/

# Sync only text files, excluding temporary ones
azst sync --include-pattern "*.txt;*.md" --exclude-pattern "*~;*.tmp" \
  /documents/ az://myaccount/docs/
```

**Pattern syntax:**
- Use wildcards: `*` (matches any characters) and `?` (matches single character)
- Separate multiple patterns with semicolons: `*.jpg;*.png;*.gif`
- Patterns are matched against the relative path of files

### Environment Variables

You can tune AzCopy's behavior using environment variables:

```bash
# Control parallel transfers (default: auto-calculated)
export AZCOPY_CONCURRENCY_VALUE=16
export AZCOPY_CONCURRENT_FILES=100

# Control memory buffer size (in GB)
export AZCOPY_BUFFER_GB=2

# Customize log locations
export AZCOPY_LOG_LOCATION=/tmp/azcopy-logs
export AZCOPY_JOB_PLAN_LOCATION=/tmp/azcopy-plans

# Run azst commands with custom settings
azst cp -r /large/dataset/ az://myaccount/data/
```

See `azcopy env` for a complete list of environment variables.

## Configuration

The tool uses the Azure CLI configuration and authentication. Make sure to:

1. Login: `az login`
2. Set default subscription: `az account set --subscription <subscription-id>`

## Examples

### Upload a website

```bash
# Upload entire website directory
azst cp -r ./dist/ az://myaccount/my-website-container/

# Upload with public access (container must be configured for public access)
azst cp -r ./public/ az://myaccount/cdn-container/assets/
```

### Backup and sync

```bash
# Backup local directory to Azure
azst cp -r /important/data/ az://myaccount/backup-container/$(date +%Y-%m-%d)/

# Download backup
azst cp -r az://myaccount/backup-container/2024-01-15/ /restore/location/
```

### Batch operations

```bash
# Remove all logs older than a certain date
azst ls az://myaccount/logs-container/2024-01/ | grep "2024-01-0[1-5]" | xargs -I {} azst rm {}

# Copy multiple files
find /local/files -name "*.txt" | xargs -I {} azst cp {} az://myaccount/text-files/
```

## Error Handling

The tool provides clear error messages for common issues:

- Azure CLI not installed or not logged in
- Invalid URI formats
- Permission issues
- Network connectivity problems
- Storage account configuration issues

## Performance

- Parallel uploads/downloads (configurable with `-j` flag)
- Efficient streaming for large files
- Progress indicators for long operations
- Optimized for both single files and bulk operations

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

This project is licensed under the MIT or Apache-2.0 license.

## Comparison with gsutil

| gsutil      | azst           | Description         |
| ----------- | -------------- | ------------------- |
| `gs://`     | `az://`        | URI scheme          |
| `gsutil cp` | `azst cp`      | Copy files          |
| `gsutil ls` | `azst ls`      | List objects        |
| `gsutil rm` | `azst rm`      | Remove objects      |
| `gsutil du` | `azst du`      | Disk usage stats    |
| `gsutil -m` | `azst cp -j N` | Parallel operations |

The tool aims to provide familiar gsutil-like semantics for Azure Blob Storage
operations.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE.md)
file for details.
