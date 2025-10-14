# azst - Azure Storage Tool

[![CI](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml)
[![Release](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A Rust CLI tool that wraps the Azure CLI to provide easier Azure Blob Storage management with `gsutil`-like semantics.

## Features

- **cp** - Copy files to/from Azure storage with `cp` semantics
- **ls** - List objects in Azure storage with detailed information
- **rm** - Remove objects from Azure storage
- **mb** - Create storage containers (make bucket)
- **rb** - Remove storage containers (remove bucket)
- **Recursive operations** with `-r` flag
- **Human-readable file sizes** with `-h` flag
- **Parallel uploads/downloads** for better performance
- **Azure URI format**: `az://<account>/<container>/path/to/object`

## Prerequisites

1. **Azure CLI**: Install from [https://docs.microsoft.com/en-us/cli/azure/install-azure-cli](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli)
2. **Authentication**: Run `az login` to authenticate with Azure
3. **Storage Account**: Configure default storage account or use `--account` flag

## Installation

### Quick Install (Recommended)

Install the latest build using curl:

```bash
curl -sSL https://raw.githubusercontent.com/dymaxionlabs/azst/main/install.sh | bash
```

This will download and install the latest binary from the `main` branch for your system to `~/.local/bin/`.

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

The binary will be installed to `~/.cargo/bin/azst` (make sure this directory is in your PATH).

## Usage

### Basic Commands

```bash
# List all containers
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

# Create container
azst mb az://myaccount/new-container

# Remove container
azst rb az://myaccount/old-container
```

### Advanced Usage

```bash
# Copy with parallel uploads (4 parallel by default)
azst cp -r -j 8 /large/directory/ az://mycontainer/

# Force operations without confirmation
azst rm -rf az://myaccount/mycontainer/prefix/
azst rb -f az://myaccount/container-to-delete

# List with human-readable sizes
azst ls -lh az://myaccount/mycontainer/

# Use specific storage account
azst mb az://mystorageaccount/new-container
```

### URI Format

Azure URIs follow the format: `az://<storage-account>/<container>/path/to/object`

This convention is specific to `azst` and provides a self-contained way to reference Azure storage resources:

- `az://myaccount/` - List all containers in storage account
- `az://myaccount/mycontainer/` - List all objects in container
- `az://myaccount/mycontainer/prefix/` - List objects with prefix
- `az://myaccount/mycontainer/file.txt` - Specific object

**Legacy format** (without storage account) is also supported for backward compatibility:
- `az://mycontainer/` - Requires `--account` flag or default configuration

**Note:** The `az://` URI scheme is not used by Microsoft Azure services, so there are no conflicts with official tools.

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

| gsutil      | azst           | Description             |
| ----------- | -------------- | ----------------------- |
| `gs://`     | `az://`        | URI scheme              |
| `gsutil cp` | `azst cp`      | Copy files              |
| `gsutil ls` | `azst ls`      | List objects            |
| `gsutil rm` | `azst rm`      | Remove objects          |
| `gsutil mb` | `azst mb`      | Make bucket/container   |
| `gsutil rb` | `azst rb`      | Remove bucket/container |
| `gsutil -m` | `azst cp -j N` | Parallel operations     |

The tool aims to provide familiar gsutil-like semantics for Azure Blob Storage operations.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE.md)
file for details.
