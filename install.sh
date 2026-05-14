#!/bin/bash
set -e

# Kumo Installer for Linux/macOS
echo "Installing Kumo Package Manager..."

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
INSTALL_DIR="$HOME/.kumo/bin"
REPO_URL="https://github.com/jmaxdev/Kumo/releases/latest/download"

if [ "$OS" == "darwin" ]; then
    FILENAME="kumo-macos.tar.gz"
else
    FILENAME="kumo-linux.tar.gz"
fi

mkdir -p "$INSTALL_DIR"

TEMP_DIR=$(mktemp -d)
echo "Downloading $FILENAME..."
curl -L "$REPO_URL/$FILENAME" -o "$TEMP_DIR/$FILENAME"

echo "Extracting..."
tar -xzf "$TEMP_DIR/$FILENAME" -C "$TEMP_DIR"

# Find and move binaries regardless of structure
find "$TEMP_DIR" -type f \( -name "kumo" -o -name "kx" \) -exec mv {} "$INSTALL_DIR/" \;

chmod +x "$INSTALL_DIR/kumo"
chmod +x "$INSTALL_DIR/kx"

# Cleanup
rm -rf "$TEMP_DIR"

echo "Kumo and KX installed successfully in $INSTALL_DIR"
echo ""
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo "Add this to your .bashrc, .zshrc or .profile:"
    echo "export PATH=\"\$PATH:$INSTALL_DIR\""
fi
