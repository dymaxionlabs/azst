#!/usr/bin/env bash
set -e

# azst installation script
# Usage: curl -sSL https://raw.githubusercontent.com/dymaxionlabs/azst/main/install.sh | bash

REPO="dymaxionlabs/azst"
BINARY_NAME="azst"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

echo -e "${GREEN}azst installer${NC}"
echo "Installing latest build from main branch"
echo ""

case "$OS" in
    Linux*)
        OS_TYPE="linux"
        ;;
    Darwin*)
        OS_TYPE="macos"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        OS_TYPE="windows"
        ;;
    *)
        echo -e "${RED}Error: Unsupported operating system: $OS${NC}"
        exit 1
        ;;
esac

case "$ARCH" in
    x86_64|amd64)
        ARCH_TYPE="x86_64"
        ;;
    aarch64|arm64)
        ARCH_TYPE="aarch64"
        ;;
    *)
        echo -e "${RED}Error: Unsupported architecture: $ARCH${NC}"
        exit 1
        ;;
esac

# Construct download URL
if [ "$OS_TYPE" = "windows" ]; then
    ARCHIVE_NAME="${BINARY_NAME}-${OS_TYPE}-${ARCH_TYPE}.exe.zip"
    ARCHIVE_EXT="zip"
else
    ARCHIVE_NAME="${BINARY_NAME}-${OS_TYPE}-${ARCH_TYPE}.tar.gz"
    ARCHIVE_EXT="tar.gz"
fi

# Get latest release version
echo "Fetching latest release..."

# Always use the 'latest' tag
LATEST_VERSION="latest"
echo "Using latest build from main branch"

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_VERSION}/${ARCHIVE_NAME}"

echo "Downloading from: $DOWNLOAD_URL"

# Create temporary directory
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

# Download archive
if ! curl -sSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE_NAME"; then
    echo -e "${RED}Error: Failed to download $ARCHIVE_NAME${NC}"
    exit 1
fi

# Extract archive
echo "Extracting..."
cd "$TMP_DIR"

if [ "$ARCHIVE_EXT" = "tar.gz" ]; then
    tar xzf "$ARCHIVE_NAME"
elif [ "$ARCHIVE_EXT" = "zip" ]; then
    unzip -q "$ARCHIVE_NAME"
fi

# Create installation directory if it doesn't exist
mkdir -p "$INSTALL_DIR"

# Install binary
if [ "$OS_TYPE" = "windows" ]; then
    BINARY_FILE="${BINARY_NAME}.exe"
else
    BINARY_FILE="$BINARY_NAME"
fi

echo "Installing to $INSTALL_DIR/$BINARY_FILE..."
mv "$BINARY_FILE" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY_FILE"

# Check if install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo -e "${YELLOW}Warning: $INSTALL_DIR is not in your PATH${NC}"
    echo "Add the following line to your shell configuration file:"
    echo ""
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
    echo ""

    # Suggest the appropriate config file
    if [ -n "$BASH_VERSION" ]; then
        echo "(For bash, add to ~/.bashrc or ~/.bash_profile)"
    elif [ -n "$ZSH_VERSION" ]; then
        echo "(For zsh, add to ~/.zshrc)"
    fi
fi

# Download and install AzCopy
echo "Checking for AzCopy..."

AZCOPY_VERSION="10.30.1"
AZCOPY_DIR="$HOME/.local/share/azst/azcopy"
# Set appropriate binary name based on OS
if [ "$OS_TYPE" = "windows" ]; then
    AZCOPY_BINARY="$AZCOPY_DIR/azcopy.exe"
else
    AZCOPY_BINARY="$AZCOPY_DIR/azcopy"
fi

# Check if we already have the correct AzCopy version
AZCOPY_NEEDS_INSTALL=true
if [ -x "$AZCOPY_BINARY" ]; then
    CURRENT_VERSION=$("$AZCOPY_BINARY" --version 2>/dev/null | head -n1 | awk '{print $3}' || echo "")
    if [ "$CURRENT_VERSION" = "$AZCOPY_VERSION" ]; then
        echo "AzCopy $AZCOPY_VERSION already installed"
        AZCOPY_NEEDS_INSTALL=false
    fi
fi

# Install AzCopy if needed
if [ "$AZCOPY_NEEDS_INSTALL" = true ]; then
    echo "Installing AzCopy $AZCOPY_VERSION..."

    # Determine AzCopy download URL based on OS and architecture
    case "$OS_TYPE" in
        linux)
            case "$ARCH_TYPE" in
                x86_64)
                    AZCOPY_URL="https://github.com/Azure/azure-storage-azcopy/releases/download/v${AZCOPY_VERSION}/azcopy_linux_amd64_${AZCOPY_VERSION}.tar.gz"
                    ;;
                aarch64)
                    AZCOPY_URL="https://github.com/Azure/azure-storage-azcopy/releases/download/v${AZCOPY_VERSION}/azcopy_linux_arm64_${AZCOPY_VERSION}.tar.gz"
                    ;;
                *)
                    echo -e "${YELLOW}Warning: Unsupported architecture $ARCH_TYPE for AzCopy. You may need to install AzCopy manually.${NC}"
                    AZCOPY_NEEDS_INSTALL=false
                    ;;
            esac
            ;;
        macos)
            case "$ARCH_TYPE" in
                x86_64)
                    AZCOPY_URL="https://github.com/Azure/azure-storage-azcopy/releases/download/v${AZCOPY_VERSION}/azcopy_darwin_amd64_${AZCOPY_VERSION}.zip"
                    ;;
                aarch64)
                    AZCOPY_URL="https://github.com/Azure/azure-storage-azcopy/releases/download/v${AZCOPY_VERSION}/azcopy_darwin_arm64_${AZCOPY_VERSION}.zip"
                    ;;
                *)
                    echo -e "${YELLOW}Warning: Unsupported architecture $ARCH_TYPE for AzCopy. You may need to install AzCopy manually.${NC}"
                    AZCOPY_NEEDS_INSTALL=false
                    ;;
            esac
            ;;
        windows)
            case "$ARCH_TYPE" in
                x86_64)
                    AZCOPY_URL="https://github.com/Azure/azure-storage-azcopy/releases/download/v${AZCOPY_VERSION}/azcopy_windows_amd64_${AZCOPY_VERSION}.zip"
                    ;;
                aarch64)
                    AZCOPY_URL="https://github.com/Azure/azure-storage-azcopy/releases/download/v${AZCOPY_VERSION}/azcopy_windows_arm64_${AZCOPY_VERSION}.zip"
                    ;;
                *)
                    echo -e "${YELLOW}Warning: Unsupported architecture $ARCH_TYPE for AzCopy. You may need to install AzCopy manually.${NC}"
                    AZCOPY_NEEDS_INSTALL=false
                    ;;
            esac
            ;;
    esac

    if [ "$AZCOPY_NEEDS_INSTALL" = true ] && [ -n "$AZCOPY_URL" ]; then
        # Create AzCopy directory
        mkdir -p "$AZCOPY_DIR"

        # Download AzCopy
        AZCOPY_ARCHIVE_NAME=$(basename "$AZCOPY_URL")
        echo "Downloading AzCopy from: $AZCOPY_URL"

        if ! curl -sSL "$AZCOPY_URL" -o "$TMP_DIR/$AZCOPY_ARCHIVE_NAME"; then
            echo -e "${YELLOW}Warning: Failed to download AzCopy. You may need to install it manually from https://aka.ms/downloadazcopy${NC}"
        else
            # Extract AzCopy
            cd "$TMP_DIR"
            if [[ "$AZCOPY_ARCHIVE_NAME" == *.tar.gz ]]; then
                tar xzf "$AZCOPY_ARCHIVE_NAME"
                # Find the azcopy binary (it's usually in a subdirectory)
                AZCOPY_EXTRACTED=$(find . -name "azcopy" -type f | head -n1)
            elif [[ "$AZCOPY_ARCHIVE_NAME" == *.zip ]]; then
                unzip -q "$AZCOPY_ARCHIVE_NAME"
                # Find the azcopy binary (it's usually in a subdirectory)
                # On Windows, look for azcopy.exe, otherwise look for azcopy
                if [ "$OS_TYPE" = "windows" ]; then
                    AZCOPY_EXTRACTED=$(find . -name "azcopy.exe" -type f | head -n1)
                else
                    AZCOPY_EXTRACTED=$(find . -name "azcopy" -type f | head -n1)
                fi
            fi

            if [ -n "$AZCOPY_EXTRACTED" ] && [ -f "$AZCOPY_EXTRACTED" ]; then
                # Install the binary
                cp "$AZCOPY_EXTRACTED" "$AZCOPY_BINARY"
                chmod +x "$AZCOPY_BINARY"
                echo -e "${GREEN}✓ AzCopy $AZCOPY_VERSION installed successfully${NC}"
            else
                echo -e "${YELLOW}Warning: Could not find azcopy binary in downloaded archive${NC}"
            fi
        fi
    fi
fi

echo ""
echo -e "${GREEN}✓ Installation complete!${NC}"
echo ""
echo "Run 'azst --help' to get started"

# Verify installation
if command -v "$BINARY_NAME" >/dev/null 2>&1; then
    echo ""
    echo "Installed version:"
    "$BINARY_NAME" --version
else
    echo ""
    echo "Note: You may need to restart your shell or run:"
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
fi
