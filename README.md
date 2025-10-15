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

**Note**: The installation script automatically downloads AzCopy v10.30.1 for Linux, macOS, and Windows (when run in Git Bash, WSL, or similar shell environments). For manual installation, AzCopy can be installed separately from [https://aka.ms/downloadazcopy](https://aka.ms/downloadazcopy).

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

Run `azst --help` to see all available commands and options.

For detailed help on any command, use:
```bash
azst <command> --help
```

### Quick Examples

```bash
# List storage accounts
azst ls

# List containers in an account
azst ls az://myaccount/

# Copy to Azure
azst cp -r /local/dir/ az://myaccount/mycontainer/

# Download from Azure
azst cp -r az://myaccount/mycontainer/data/ /local/backup/

# Remove files
azst rm -r az://myaccount/mycontainer/old-files/
```

### URI Format

Azure URIs follow the format:
`az://<storage-account>/<container>/path/to/object`

Examples:
- `az://myaccount/` - List all containers in storage account
- `az://myaccount/mycontainer/` - List all objects in container
- `az://myaccount/mycontainer/prefix/` - List objects with prefix
- `az://myaccount/mycontainer/file.txt` - Specific object

**Note:** The `az://` URI scheme is specific to `azst` and is not used by official Microsoft Azure tools.

## Configuration

The tool uses the Azure CLI configuration and authentication:

1. Login: `az login`
2. Set default subscription: `az account set --subscription <subscription-id>`

## Performance

- Uses AzCopy backend for blazing-fast transfers
- Parallel uploads/downloads (configurable)
- Efficient streaming for large files
- Azure-to-Azure copies are server-side (no local transfer)

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

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE.md)
file for details.
