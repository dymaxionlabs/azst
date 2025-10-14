# azst - Azure Storage Tool

[![CI](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml)
[![Release](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

CLI tool for Azure Blob Storage with `gsutil`-like semantics. Uses **AzCopy** as
backend for blazing-fast transfers while providing a clean, intuitive interface.

## Features

- **🚀 High Performance** - Uses AzCopy backend for maximum transfer speeds
- **cp** - Copy files to/from/between Azure storage (including Azure-to-Azure
  server-side copies)
- **ls** - List objects in Azure storage with detailed information
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

1. **AzCopy**: Install from [https://aka.ms/downloadazcopy](https://aka.ms/downloadazcopy)
2. **Azure CLI**: Install from [https://docs.microsoft.com/en-us/cli/azure/install-azure-cli](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli)
3. **Authentication**: Run `az login` to authenticate with Azure

## Installation

### Quick Install (Recommended)

Install the latest build using curl:

```bash
curl -sSL https://raw.githubusercontent.com/dymaxionlabs/azst/main/install.sh | bash
```

This will download and install the latest binary from the `main` branch for your
system to `~/.local/bin/`.

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
| `gsutil -m` | `azst cp -j N` | Parallel operations |

The tool aims to provide familiar gsutil-like semantics for Azure Blob Storage
operations.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE.md)
file for details.
