#!/bin/bash
set -e

# NabiMan Installer
# Usage: curl -sSL https://raw.githubusercontent.com/matrixism-cmyk/NabiMan/main/install.sh | bash

REPO="matrixism-cmyk/NabiMan"
INSTALL_DIR="/usr/local/bin"
STATIC_DIR="/usr/local/share/nabiman/static"
DATA_DIR="/var/lib/nabiman"
SERVICE_FILE="/etc/systemd/system/nabiman.service"

echo "=== NabiMan Installer ==="

# Check root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: Please run as root (sudo)"
    exit 1
fi

# Check dependencies
for cmd in curl; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: $cmd is required but not installed"
        exit 1
    fi
done

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    aarch64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Get latest release
echo "Downloading latest release..."
RELEASE_URL="https://api.github.com/repos/$REPO/releases/latest"
DOWNLOAD_URL=$(curl -sSL "$RELEASE_URL" | grep "browser_download_url.*nabiman-$ARCH" | head -1 | cut -d'"' -f4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "No pre-built binary found. Building from source..."
    echo "Please install Rust and Node.js, then run: make install"
    exit 1
fi

curl -sSL "$DOWNLOAD_URL" -o /tmp/nabiman-server
chmod +x /tmp/nabiman-server

# Install
mkdir -p "$INSTALL_DIR" "$STATIC_DIR" "$DATA_DIR"
mv /tmp/nabiman-server "$INSTALL_DIR/"

# Create systemd service
cat > "$SERVICE_FILE" << EOF
[Unit]
Description=NabiMan Server Management Dashboard
After=network.target

[Service]
Type=simple
User=root
ExecStart=$INSTALL_DIR/nabiman-server
Environment=NABIMAN_PORT=8080
Environment=NABIMAN_STATIC=$STATIC_DIR
Environment=NABIMAN_DATA_DIR=$DATA_DIR
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable nabiman

echo ""
echo "=== NabiMan installed ==="
echo "Start:   systemctl start nabiman"
echo "Status:  systemctl status nabiman"
echo "Access:  http://$(hostname -I | awk '{print $1}'):8080"
echo "Default password: nabiman (change immediately!)"
