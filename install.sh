#!/bin/bash
set -e

# Kumo Installer for Linux/macOS
echo "🚀 Installing Kumo Package Manager..."

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

echo "📥 Downloading $FILENAME..."
curl -L "$REPO_URL/$FILENAME" -o "/tmp/$FILENAME"

echo "📦 Extracting..."
tar -xzf "/tmp/$FILENAME" -C "$INSTALL_DIR"

chmod +x "$INSTALL_DIR/kumo"
chmod +x "$INSTALL_DIR/kx"

echo "✨ Kumo installed successfully in $INSTALL_DIR"
echo ""
echo "💡 Add this to your .bashrc or .zshrc:"
echo "export PATH=\"\$PATH:\$HOME/.kumo/bin\""
