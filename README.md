# azst

[![CI](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/ci.yml)
[![Release](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml/badge.svg)](https://github.com/dymaxionlabs/azst/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

CLI tool for Azure Blob Storage with POSIX-like semantics. Uses **AzCopy** as
backend for blazing-fast transfers while providing a clean, intuitive interface.

## Features

- **🔧 Complete Toolset** - `cat`, `cp`, `ls`, `du`, `mv`, `rm`, and `sync`
  commands
- **🎯 Clean URI Syntax** - `az://account/container/path` instead of verbose
  HTTPS URLs
- **⚡ High Performance** - AzCopy backend with parallel transfers and
  server-side copies
- **🔍 Pattern Matching** - Glob patterns (`*.txt`, `**/*.jpg`) and wildcards
  for filtering
- **🔒 Safe Operations** - Dry-run mode, confirmation prompts, and detailed
  previews
- **🎨 Better UX** - Colored output, progress indicators, and clear error
  messages

## Why azst?

AzCopy is fast and Azure CLI is comprehensive, but both require learning
Azure-specific syntax and working with verbose HTTPS URLs.

`azst` offers a simpler alternative:
- **Familiar commands** - `cp`, `ls`, `rm`, `du` work like their Unix
  counterparts
- **Shorter URIs** - `az://account/container/path` vs
  `https://account.blob.core.windows.net/container/path`
- **Same speed** - Uses AzCopy under the hood for parallel transfers
- **No new auth** - Works with your existing `az login` credentials

Ideal for developers who prefer Unix-style tools or are migrating from GCP's
`gsutil`.

## Prerequisites

### For Local Development
- **Azure CLI**: Install from [https://docs.microsoft.com/en-us/cli/azure/install-azure-cli](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli)
- **Authentication**: Run `az login` to authenticate

### For Production / Azure VMs
No additional prerequisites! `azst` automatically detects:
- **Managed Identity** on Azure VMs, App Service, AKS, Container Instances
- **Service Principal** credentials via environment variables:
  - `AZURE_TENANT_ID`
  - `AZURE_CLIENT_ID`
  - `AZURE_CLIENT_SECRET`

### Credential Chain
`azst` tries authentication methods in this order:
1. **Environment Variables** - Service Principal (AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET)
2. **Managed Identity** - Automatic on Azure VMs and services
3. **Azure CLI** - Uses `az login` credentials for local development

**Note**: AzCopy will be automatically downloaded and installed during first use.

## Installation

### Quick Install (Recommended)

**macOS / Linux / WSL:**
```bash
curl -sSL https://raw.githubusercontent.com/dymaxionlabs/azst/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/dymaxionlabs/azst/main/install.ps1 | iex
```

This will download and install:
- The latest `azst` binary from the `main` branch for your system
- AzCopy version 10.30.1 (if not already present)

**Installation locations:**
- Linux/macOS: `~/.local/bin/azst`
- Windows: `%LOCALAPPDATA%\Programs\azst\azst.exe`

### Manual Installation

Download the latest build for your platform from the
[releases page](https://github.com/dymaxionlabs/azst/releases/tag/latest).

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

**Using PowerShell:**
```powershell
# Extract to a directory
Expand-Archive -Path azst-windows-x86_64.exe.zip -DestinationPath $env:LOCALAPPDATA\Programs\azst

# Add to PATH (run as administrator or add to User PATH)
$Path = [Environment]::GetEnvironmentVariable("Path", "User")
[Environment]::SetEnvironmentVariable("Path", "$Path;$env:LOCALAPPDATA\Programs\azst", "User")
```

**Or using File Explorer:**
1. Extract the .zip file to a folder (e.g., `C:\Program Files\azst`)
2. Add that folder to your system PATH:
   - Search for "Environment Variables" in Windows
   - Edit the "Path" variable
   - Add the folder path

**Note**: AzCopy will be automatically downloaded and installed during the
installation process for all platforms.

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

**Note:** The `az://` URI scheme is specific to `azst` and is not used by
official Microsoft Azure tools.

## Configuration

The tool uses the Azure CLI configuration and authentication:

1. Login: `az login`
2. Set default subscription: `az account set --subscription <subscription-id>`

## Architecture

`azst` uses a hybrid approach for optimal performance and flexibility:

### Authentication
Uses Azure SDK's credential chain (similar to AzCopy):
- **Local Development**: Automatically uses `az login` credentials
- **Azure VMs/Services**: Automatically uses Managed Identity
- **CI/CD Pipelines**: Uses Service Principal from environment variables

No code changes needed when deploying to Azure! Set `AZURE_CREDENTIAL_KIND` 
environment variable to force a specific credential type:
- `azurecli` - Azure CLI only
- `virtualmachine` - Managed Identity only
- `environment` - Environment variables only

### Operations
**Read Operations** (`ls`, `cat`, `du`):
- Uses the **Azure SDK for Rust** for direct API calls
- No subprocess overhead, better performance
- Streaming support for efficient memory usage

**Write Operations** (`cp`, `sync`, `mv`, `rm`):
- Uses **AzCopy** for maximum throughput
- Parallel transfers and server-side copies
- Battle-tested for production workloads

## Performance

- Uses AzCopy backend for blazing-fast transfers
- Parallel uploads/downloads (configurable)
- Efficient streaming for large files
- Azure-to-Azure copies are server-side (no local transfer)

## Comparison with gsutil

| gsutil         | azst        | Description      |
| -------------- | ----------- | ---------------- |
| `gs://`        | `az://`     | URI scheme       |
| `gsutil cp`    | `azst cp`   | Copy files       |
| `gsutil ls`    | `azst ls`   | List objects     |
| `gsutil rm`    | `azst rm`   | Remove objects   |
| `gsutil du`    | `azst du`   | Disk usage stats |
| `gsutil rsync` | `azst sync` | Sync directories |

The tool aims to provide familiar gsutil-like semantics for Azure Blob Storage
operations. All copy and sync operations use AzCopy for parallel transfers by
default.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE.md)
file for details.
