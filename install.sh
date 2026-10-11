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
for cmd in curl tar sha256sum mktemp python3; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: $cmd is required but not installed"
        exit 1
    fi
done

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    *) echo "No release package is available for architecture: $ARCH" >&2; exit 1 ;;
esac

# Get latest release
echo "Downloading latest release package..."
ASSET="nabiman-linux-$ARCH.tar.gz"
RELEASE_BASE="https://github.com/$REPO/releases/latest/download"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
curl -fL --retry 3 --silent --show-error "$RELEASE_BASE/$ASSET" -o "$STAGE/$ASSET"
curl -fL --retry 3 --silent --show-error "$RELEASE_BASE/$ASSET.sha256" -o "$STAGE/$ASSET.sha256"
(cd "$STAGE" && sha256sum -c "$ASSET.sha256")
# The verifier is trusted publisher code, like this installer; this is not signature verification.
# New release format is required. Do not fall back to unverified tar extraction.
curl -fL --retry 3 --silent --show-error "$RELEASE_BASE/verify-release.py" -o "$STAGE/verify-release.py"
python3 "$STAGE/verify-release.py" "$STAGE/$ASSET" --extract-to "$STAGE/verified"
PAYLOAD="$STAGE/verified/nabiman"
if [ ! -x "$PAYLOAD/nabiman-server" ] || [ ! -f "$PAYLOAD/static/index.html" ]; then
    echo "Release package is missing the server or web interface." >&2
    exit 1
fi

if [ ! -e "$DATA_DIR/users.json" ]; then
    BOOTSTRAP_PASSWORD=${NABIMAN_PASSWORD:-}
    if [ -z "$BOOTSTRAP_PASSWORD" ]; then
        if [ ! -r /dev/tty ]; then
            echo "Set NABIMAN_PASSWORD before installation (12+ characters)." >&2
            exit 1
        fi
        read -r -s -p "Initial admin password (12+ characters): " BOOTSTRAP_PASSWORD < /dev/tty
        echo > /dev/tty
    fi
    if [ "${#BOOTSTRAP_PASSWORD}" -lt 12 ] || [ "$BOOTSTRAP_PASSWORD" = "nabiman" ] || [ "$BOOTSTRAP_PASSWORD" = "changeme" ]; then
        echo "Initial admin password must be unique and at least 12 characters." >&2
        exit 1
    fi
fi

# Install
mkdir -p "$INSTALL_DIR" "$(dirname "$STATIC_DIR")" "$DATA_DIR"
BIN_STAGE=$(mktemp "$INSTALL_DIR/.nabiman-server.XXXXXX")
STATIC_STAGE=$(mktemp -d "$(dirname "$STATIC_DIR")/.nabiman-static.XXXXXX")
install -m 755 "$PAYLOAD/nabiman-server" "$BIN_STAGE"
cp -a "$PAYLOAD/static/." "$STATIC_STAGE/"

STATIC_BACKUP=""
if [ -d "$STATIC_DIR" ]; then
    STATIC_BACKUP=$(mktemp -d "$(dirname "$STATIC_DIR")/.nabiman-static-backup.XXXXXX")
    rmdir "$STATIC_BACKUP"
    mv "$STATIC_DIR" "$STATIC_BACKUP"
fi
if ! mv "$STATIC_STAGE" "$STATIC_DIR"; then
    if [ -n "$STATIC_BACKUP" ]; then mv "$STATIC_BACKUP" "$STATIC_DIR"; fi
    echo "Could not install web interface." >&2
    exit 1
fi
if ! mv -f "$BIN_STAGE" "$INSTALL_DIR/nabiman-server"; then
    rm -rf "$STATIC_DIR"
    if [ -n "$STATIC_BACKUP" ]; then mv "$STATIC_BACKUP" "$STATIC_DIR"; fi
    echo "Could not install server binary." >&2
    exit 1
fi
for RECOVERY_TOOL in nabiman-recovery management-recovery.py; do
    RECOVERY_STAGE=$(mktemp "$INSTALL_DIR/.recovery-tool.XXXXXX")
    install -m 755 "$PAYLOAD/$RECOVERY_TOOL" "$RECOVERY_STAGE"
    mv -f "$RECOVERY_STAGE" "$INSTALL_DIR/$RECOVERY_TOOL"
done
if [ -n "$STATIC_BACKUP" ]; then rm -rf "$STATIC_BACKUP"; fi
if [ -n "${BOOTSTRAP_PASSWORD:-}" ]; then
    install -m 600 /dev/null "$DATA_DIR/password"
    printf '%s' "$BOOTSTRAP_PASSWORD" > "$DATA_DIR/password"
    unset BOOTSTRAP_PASSWORD
fi

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
EnvironmentFile=-/etc/nabiman/nabiman.env
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
echo "Start/restart: systemctl restart nabiman"
echo "Status:  systemctl status nabiman"
echo "Access:  http://$(hostname -I | awk '{print $1}'):8080"
echo "Login with the configured admin password."
