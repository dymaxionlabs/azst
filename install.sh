#!/usr/bin/env bash
set -e

# azst installation script
# Usage: curl -sSL https://raw.githubusercontent.com/munshkr/azst/main/install.sh | bash

REPO="munshkr/azst"
BINARY_NAME="azst"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

echo -e "${GREEN}azst installer${NC}"
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
LATEST_VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_VERSION" ]; then
    echo -e "${RED}Error: Could not determine latest version${NC}"
    exit 1
fi

echo "Latest version: $LATEST_VERSION"

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
