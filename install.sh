#!/bin/bash
set -euo pipefail

# NabiMan Installer
#
# Usage (as root):
#   curl -fsSLO https://github.com/matrixism-cmyk/NabiMan/releases/latest/download/install.sh
#   sudo bash install.sh [options]
#
# Download from a release, not from a branch: the release's install.sh carries
# the release signing key it checks the package with. Verify install.sh itself
# against the key published in docs/release/ (see docs/release/README.md).
#
# Options:
#   --version vX.Y.Z         install this release (default: the latest)
#   --base-url URL           download from URL/<asset> instead of GitHub releases
#                            (a mirror; the signature is still checked)
#   --archive FILE           install a package already on disk (signature: FILE.minisig)
#   --signature FILE         signature for --archive
#   --insecure-skip-verify   install even without a valid signature (NOT for production)
#   --install-deps           install missing runtime dependencies with apt-get/dnf
#   --with-optional          with --install-deps: the optional ones too
#   --mode root|service      root (default): the API runs as root, as before.
#                            service: the API runs as the `nabiman` system user
#                            (run_mode = service) with nabiman-privileged-helper
#                            (root) for service control, firewall rules and
#                            package upgrades; other root features are reported
#                            unavailable (docs/security/privilege-inventory.md)
#   -h, --help
#
# Layout (docs/release/README.md):
#   /usr/local/lib/nabiman/<version>/  each installed release; current -> the one in use
#   /usr/local/bin/nabiman-server, nabiman-privileged-helper, nabiman-recovery,
#     management-recovery.py, nabiman-update   links into current/
#   /usr/local/share/nabiman/static    link to current/static (web interface)
#   /var/lib/nabiman                   data (users, keys, databases)
#   /etc/nabiman                       nabiman.env (local settings), release.pub
#   /etc/systemd/system/nabiman.service
#   --mode service also: user nabiman, /var/lib/nabiman owned nabiman:nabiman 0700,
#     /etc/systemd/system/nabiman-privileged-helper.service (+ .d/system-operations.conf),
#     /etc/nabiman/privileged-services.json and privileged-system.json (root 0600),
#     the helper socket /run/nabiman-helper/control.sock (nabiman 0600, root-owned dir)

REPO="${NABIMAN_REPO:-matrixism-cmyk/NabiMan}"
LIB_DIR="/usr/local/lib/nabiman"
DATA_DIR="/var/lib/nabiman"
CONFIG_DIR="/etc/nabiman"
SERVICE_FILE="/etc/systemd/system/nabiman.service"

# The release signing public key (minisign). Release builds refuse to publish
# while this differs from docs/release/nabiman-release.pub, so the key that
# checks a package never comes from the download it checks.
NABIMAN_RELEASE_PUBKEY='PLACEHOLDER'

VERSION="${NABIMAN_VERSION:-latest}"
BASE_URL="${NABIMAN_RELEASE_BASE:-}"
LOCAL_ARCHIVE=""
LOCAL_SIGNATURE=""
SKIP_VERIFY=0
INSTALL_DEPS=0
WITH_OPTIONAL=0
MODE=root

usage() {
    cat <<'USAGE'
Usage: sudo bash install.sh [--version vX.Y.Z] [--base-url URL]
                            [--archive FILE [--signature FILE]]
                            [--insecure-skip-verify]
                            [--install-deps [--with-optional]]
                            [--mode root|service]
USAGE
}
while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION=${2:?--version needs a value}; shift 2 ;;
        --base-url) BASE_URL=${2:?--base-url needs a value}; shift 2 ;;
        --archive) LOCAL_ARCHIVE=${2:?--archive needs a file}; shift 2 ;;
        --signature) LOCAL_SIGNATURE=${2:?--signature needs a file}; shift 2 ;;
        --insecure-skip-verify) SKIP_VERIFY=1; shift ;;
        --install-deps) INSTALL_DEPS=1; shift ;;
        --with-optional) WITH_OPTIONAL=1; shift ;;
        --mode) MODE=${2:?--mode needs root or service}; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
    esac
done

case "$MODE" in
    root|service) ;;
    *) echo "--mode is root or service, not $MODE" >&2; exit 2 ;;
esac

echo "=== NabiMan Installer ==="

# Check root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: Please run as root (sudo)"
    exit 1
fi

# Installer needs
for cmd in curl tar mktemp python3; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: $cmd is required but not installed"
        exit 1
    fi
done

# Runtime dependencies: command -> package (apt / dnf).
#   required: terminals (tmux), SSH sessions and keys, password logins, port relays
#   optional: RDP/VNC-over-Guacamole consoles, Office previews, video previews
REQUIRED_DEPS="tmux:tmux:tmux ssh:openssh-client:openssh-clients ssh-keygen:openssh-client:openssh-clients sshpass:sshpass:sshpass socat:socat:socat"
OPTIONAL_DEPS="guacd:guacd:guacd soffice:libreoffice-core:libreoffice-core ffmpeg:ffmpeg:ffmpeg-free"
if command -v apt-get >/dev/null; then PKG=apt; elif command -v dnf >/dev/null; then PKG=dnf; else PKG=""; fi
missing_packages() { # list -> packages for this host's package manager, missing commands only
    local entry cmd apt dnf out=""
    for entry in $1; do
        IFS=: read -r cmd apt dnf <<< "$entry"
        command -v "$cmd" >/dev/null && continue
        case "$PKG" in apt) out+=" $apt" ;; dnf) out+=" $dnf" ;; *) out+=" $cmd" ;; esac
    done
    echo "$out" | tr ' ' '\n' | sort -u | tr '\n' ' ' | sed 's/^ *//; s/ *$//'
}
install_packages() {
    case "$PKG" in
        apt) DEBIAN_FRONTEND=noninteractive apt-get update -q && DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends "$@" ;;
        dnf) dnf install -y "$@" ;;
        *) echo "No apt-get or dnf: install $* yourself." >&2; return 1 ;;
    esac
}
MISSING=$(missing_packages "$REQUIRED_DEPS")
MISSING_OPTIONAL=$(missing_packages "$OPTIONAL_DEPS")
if [ -n "$MISSING" ]; then
    if [ "$INSTALL_DEPS" -eq 1 ]; then
        echo "Installing runtime dependencies: $MISSING"
        # shellcheck disable=SC2086
        install_packages $MISSING
    else
        echo "Missing runtime dependencies. Install them, or rerun with --install-deps:" >&2
        case "$PKG" in
            apt) echo "  apt-get install -y $MISSING" >&2 ;;
            dnf) echo "  dnf install -y $MISSING    (sshpass: EPEL on RHEL/Rocky/Alma)" >&2 ;;
            *) echo "  commands: $MISSING" >&2 ;;
        esac
        exit 1
    fi
fi
if [ -n "$MISSING_OPTIONAL" ]; then
    if [ "$INSTALL_DEPS" -eq 1 ] && [ "$WITH_OPTIONAL" -eq 1 ]; then
        echo "Installing optional dependencies: $MISSING_OPTIONAL"
        # shellcheck disable=SC2086
        install_packages $MISSING_OPTIONAL || echo "Some optional packages could not be installed; the features that need them stay off." >&2
    else
        echo "Optional, for RDP consoles / Office previews / video previews: $MISSING_OPTIONAL"
        echo "  (install later, or rerun with --install-deps --with-optional)"
    fi
fi

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "No release package is available for architecture: $ARCH" >&2; exit 1 ;;
esac

ASSET="nabiman-linux-$ARCH.tar.gz"
if [ -z "$BASE_URL" ]; then
    if [ "$VERSION" = "latest" ]; then
        BASE_URL="https://github.com/$REPO/releases/latest/download"
    else
        BASE_URL="https://github.com/$REPO/releases/download/$VERSION"
    fi
fi
# Staged next to the versions, so the verified package moves into place in one rename.
mkdir -p "$LIB_DIR"
STAGE=$(mktemp -d "$LIB_DIR/.install.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT
if [ -n "$LOCAL_ARCHIVE" ]; then
    cp "$LOCAL_ARCHIVE" "$STAGE/$ASSET"
    LOCAL_SIGNATURE=${LOCAL_SIGNATURE:-$LOCAL_ARCHIVE.minisig}
    if [ -f "$LOCAL_SIGNATURE" ]; then cp "$LOCAL_SIGNATURE" "$STAGE/$ASSET.minisig"; fi
else
    echo "Downloading $ASSET ($VERSION) from $BASE_URL ..."
    curl -fL --retry 3 --silent --show-error "$BASE_URL/$ASSET" -o "$STAGE/$ASSET"
    curl -fL --retry 3 --silent --show-error "$BASE_URL/$ASSET.minisig" -o "$STAGE/$ASSET.minisig" || true
fi

# The signature is checked with the key above and the verifier below, both part
# of this script: nothing fetched with the package is trusted before it passes.
verify_signature() {
    NABIMAN_RELEASE_PUBKEY="$NABIMAN_RELEASE_PUBKEY" python3 - "$STAGE/$ASSET" "$STAGE/$ASSET.minisig" "$ASSET" <<'PYVERIFY'
import os,sys
# --- minisign verifier begin (kept identical in install.sh; scripts/test-release-signature.py checks) ---
import base64 as _b64,hashlib as _hl
_P=2**255-19;_L=2**252+27742317777372353535851937790883648493;_D=-121665*pow(121666,_P-2,_P)%_P;_I=pow(2,(_P-1)//4,_P)
def _add(a,b):
 A=(a[1]-a[0])*(b[1]-b[0])%_P;B=(a[1]+a[0])*(b[1]+b[0])%_P;C=2*a[3]*b[3]*_D%_P;E=2*a[2]*b[2]%_P
 e,f,g,h=B-A,E-C,E+C,B+A;return (e*f%_P,g*h%_P,f*g%_P,e*h%_P)
def _mul(s,a):
 q=(0,1,1,0)
 while s:
  if s&1:q=_add(q,a)
  a=_add(a,a);s>>=1
 return q
def _point(raw):
 y=int.from_bytes(raw,'little');sign=y>>255;y&=(1<<255)-1
 if y>=_P:return None
 x2=(y*y-1)*pow(_D*y*y+1,_P-2,_P)%_P
 if x2==0:
  if sign:return None
  return (0,y,1,0)
 x=pow(x2,(_P+3)//8,_P)
 if (x*x-x2)%_P:x=x*_I%_P
 if (x*x-x2)%_P:return None
 if x&1!=sign:x=_P-x
 return (x,y,1,x*y%_P)
_GY=4*pow(5,_P-2,_P)%_P;_G=_point(_GY.to_bytes(32,'little'))
def ed25519_verify(public,message,signature):
 """RFC 8032 Ed25519 verification; uses the cryptography package when it is installed."""
 if len(public)!=32 or len(signature)!=64:return False
 try:
  from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
  from cryptography.exceptions import InvalidSignature
  try:Ed25519PublicKey.from_public_bytes(public).verify(signature,message);return True
  except InvalidSignature:return False
 except ImportError:pass
 a=_point(public);r=_point(signature[:32]);s=int.from_bytes(signature[32:],'little')
 if a is None or r is None or s>=_L:return False
 h=int.from_bytes(_hl.sha512(signature[:32]+public+message).digest(),'little')%_L
 left=_mul(s,_G);right=_add(r,_mul(h,a))
 return (left[0]*right[2]-right[0]*left[2])%_P==0 and (left[1]*right[2]-right[1]*left[2])%_P==0
def minisign_public_key(text):
 """A minisign public key: the base64 line alone or the whole .pub file. Returns (key id, key)."""
 lines=[l.strip() for l in text.strip().splitlines() if l.strip() and not l.startswith('untrusted comment:')]
 if len(lines)!=1:raise ValueError('Public key must be one minisign key')
 raw=_b64.b64decode(lines[0],validate=True)
 if len(raw)!=42 or raw[:2]!=b'Ed':raise ValueError('Not a minisign Ed25519 public key')
 return raw[2:10],raw[10:]
def minisign_verify(path,signature_text,public_key_text):
 """Checks a minisign signature over the file at path; returns its trusted comment."""
 key_id,public=minisign_public_key(public_key_text)
 lines=signature_text.splitlines()
 if len(lines)<4 or not lines[0].startswith('untrusted comment:') or not lines[2].startswith('trusted comment: '):raise ValueError('Malformed minisign signature')
 sig=_b64.b64decode(lines[1].strip(),validate=True);glob=_b64.b64decode(lines[3].strip(),validate=True)
 if len(sig)!=74 or sig[:2] not in (b'Ed',b'ED') or len(glob)!=64:raise ValueError('Unsupported minisign signature')
 if sig[2:10]!=key_id:raise ValueError('Signed by a different key (key id mismatch)')
 if sig[:2]==b'ED':
  digest=_hl.blake2b(digest_size=64)
  with open(path,'rb') as f:
   for chunk in iter(lambda:f.read(1<<20),b''):digest.update(chunk)
  message=digest.digest()
 else:
  with open(path,'rb') as f:message=f.read()
 if not ed25519_verify(public,message,sig[10:]):raise ValueError('Signature does not match the file')
 trusted=lines[2][len('trusted comment: '):]
 if not ed25519_verify(public,sig[10:]+trusted.encode(),glob):raise ValueError('Trusted comment signature is invalid')
 return trusted
# --- minisign verifier end ---
archive,signature,name=sys.argv[1:4]
try:
 key=os.environ['NABIMAN_RELEASE_PUBKEY']
 if not os.path.isfile(signature):raise ValueError('no signature file ('+name+'.minisig) was published or given')
 comment=minisign_verify(archive,open(signature).read(),key)
 fields=dict(w.split(':',1) for w in comment.split() if ':' in w)
 if fields.get('file')!=name:raise ValueError('signature is for '+repr(fields.get('file'))+', not '+name)
 print('Signature OK: '+comment)
except (ValueError,OSError) as error:
 print('Signature check failed: '+str(error),file=sys.stderr);sys.exit(1)
PYVERIFY
}
if [ "$SKIP_VERIFY" -eq 1 ]; then
    echo "WARNING: --insecure-skip-verify: the package's publisher is NOT verified." >&2
elif [ "$NABIMAN_RELEASE_PUBKEY" = "PLACEHOLDER" ]; then
    echo "This installer has no release signing key (development copy)." >&2
    echo "Use the install.sh attached to a release, or pass --insecure-skip-verify knowingly." >&2
    exit 1
elif ! verify_signature; then
    echo "Refusing to install an unsigned or mismatched package." >&2
    echo "(Pass --insecure-skip-verify only if you verified it some other way.)" >&2
    exit 1
fi

# Authenticated (or explicitly accepted): the package's own verifier now checks
# every file against its manifest and extracts into a new directory.
python3 - "$STAGE/$ASSET" "$STAGE/verify-release.py" <<'PYEXTRACT'
import sys,tarfile
with tarfile.open(sys.argv[1],'r:gz') as package:
    member=package.getmember('nabiman/verify-release.py')
    if not member.isfile() or member.size>1<<20:raise SystemExit('Package verifier is missing or malformed')
    open(sys.argv[2],'wb').write(package.extractfile(member).read())
PYEXTRACT
python3 "$STAGE/verify-release.py" "$STAGE/$ASSET" --extract-to "$STAGE/verified"
PAYLOAD="$STAGE/verified/nabiman"
if [ ! -x "$PAYLOAD/nabiman-server" ] || [ ! -f "$PAYLOAD/static/index.html" ] || [ ! -f "$PAYLOAD/nabiman.service" ] || [ ! -f "$PAYLOAD/VERSION" ]; then
    echo "Release package is missing the server, web interface or service unit." >&2
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

# Install: the package becomes /usr/local/lib/nabiman/<version>, and the fixed
# paths (/usr/local/bin/..., /usr/local/share/nabiman/static) link to
# .../current. An older unversioned install is kept as a version of its own.
# nabiman-update does the same for later releases (and rolls back).
mkdir -p "$DATA_DIR" "$CONFIG_DIR"
chmod 700 "$DATA_DIR"
bash "$PAYLOAD/packaging/nabiman-update" --from-verified-dir "$PAYLOAD" --keep 3
# Later updates check signatures with this key (not with one they download).
mkdir -p "$CONFIG_DIR"
if [ "$NABIMAN_RELEASE_PUBKEY" != "PLACEHOLDER" ]; then
    printf 'untrusted comment: NabiMan release signing key (installed by install.sh)\n%s\n' "$NABIMAN_RELEASE_PUBKEY" > "$CONFIG_DIR/release.pub"
    chmod 644 "$CONFIG_DIR/release.pub"
fi
if [ -n "${BOOTSTRAP_PASSWORD:-}" ]; then
    install -m 600 /dev/null "$DATA_DIR/password"
    printf '%s' "$BOOTSTRAP_PASSWORD" > "$DATA_DIR/password"
    unset BOOTSTRAP_PASSWORD
fi

# systemd unit, from the package (its comments explain each setting).
UNIT_SOURCE="$LIB_DIR/current/nabiman.service"
if [ "$MODE" = service ]; then
    UNIT_SOURCE="$LIB_DIR/current/packaging/nabiman-service-mode.service"
fi
if [ -f "$SERVICE_FILE" ] && ! cmp -s "$UNIT_SOURCE" "$SERVICE_FILE"; then
    cp -p "$SERVICE_FILE" "$SERVICE_FILE.bak.$(date +%Y%m%d-%H%M%S)"
fi
install -m 644 "$UNIT_SOURCE" "$SERVICE_FILE"

# Service mode: the API as `nabiman`, root work through the helper.
if [ "$MODE" = service ]; then
    if ! id -u nabiman >/dev/null 2>&1; then
        if command -v systemd-sysusers >/dev/null; then
            install -D -m 644 "$LIB_DIR/current/packaging/nabiman.sysusers.conf" /usr/lib/sysusers.d/nabiman.conf
            systemd-sysusers /usr/lib/sysusers.d/nabiman.conf
        else
            useradd --system --user-group --home-dir "$DATA_DIR" --no-create-home --shell /usr/sbin/nologin nabiman
        fi
    fi
    API_UID=$(id -u nabiman)
    if [ "$API_UID" -eq 0 ]; then
        echo "The nabiman account has uid 0; refusing to use it." >&2
        exit 1
    fi
    # Log viewer and SSH attack watch read the journal (optional group).
    if getent group systemd-journal >/dev/null; then usermod -a -G systemd-journal nabiman; fi
    # The data was root's: hand it over (0700, files 0600 stay as they are).
    chown -R nabiman:nabiman "$DATA_DIR"
    chmod 700 "$DATA_DIR"
    # /etc/nabiman stays root's; the helper refuses policy files others can change.
    chown root:root "$CONFIG_DIR"
    chmod 755 "$CONFIG_DIR"
    if [ ! -e "$CONFIG_DIR/privileged-services.json" ]; then
        install -m 600 /dev/null "$CONFIG_DIR/privileged-services.json"
        printf '{\n  "api_uid": %s,\n  "services": []\n}\n' "$API_UID" > "$CONFIG_DIR/privileged-services.json"
    fi
    if [ ! -e "$CONFIG_DIR/privileged-system.json" ]; then
        # The same reach root mode had for these two; set either to false to withdraw it.
        install -m 600 /dev/null "$CONFIG_DIR/privileged-system.json"
        printf '{\n  "api_uid": %s,\n  "firewall": true,\n  "package_upgrade": true\n}\n' "$API_UID" > "$CONFIG_DIR/privileged-system.json"
    fi
    install -m 644 "$LIB_DIR/current/packaging/nabiman-privileged-helper.service" /etc/systemd/system/nabiman-privileged-helper.service
    install -d -m 755 /etc/systemd/system/nabiman-privileged-helper.service.d
    install -m 644 "$LIB_DIR/current/packaging/nabiman-privileged-helper.service.d/system-operations.conf" \
        /etc/systemd/system/nabiman-privileged-helper.service.d/system-operations.conf
fi
if [ ! -e "$CONFIG_DIR/nabiman.env" ]; then
    install -m 600 /dev/null "$CONFIG_DIR/nabiman.env"
    printf '# Local settings for nabiman.service, e.g.\n# NABIMAN_PORT=8080\n' > "$CONFIG_DIR/nabiman.env"
fi

systemctl daemon-reload
if [ "$MODE" = service ]; then
    systemctl enable nabiman-privileged-helper
fi
systemctl enable nabiman

echo ""
echo "=== NabiMan installed ==="
echo "Start/restart: systemctl restart nabiman"
echo "Update:  nabiman-update        (signed, health-checked, rolls back by itself)"
echo "Rollback: nabiman-update --rollback"
echo "Status:  systemctl status nabiman"
if [ "$MODE" = service ]; then
    echo "Mode:    service (API as nabiman). Start the helper first: systemctl start nabiman-privileged-helper"
    echo "         Services it may control: $CONFIG_DIR/privileged-services.json (empty until you list units)."
    echo "         Terminals now run as nabiman; sessions of a previous root install do not carry over."
fi
PORT=$(sed -n 's/^NABIMAN_PORT=//p' "$CONFIG_DIR/nabiman.env" | tail -n1)
LISTEN=$(sed -n 's/^NABIMAN_LISTEN=//p' "$CONFIG_DIR/nabiman.env" | tail -n1)
if [ -n "$LISTEN" ]; then
    echo "Access:  listening on $LISTEN (NABIMAN_LISTEN in $CONFIG_DIR/nabiman.env)"
else
    echo "Access:  http://127.0.0.1:${PORT:-8080} on this machine only. Put a TLS proxy (nginx)"
    echo "         in front, or set NABIMAN_LISTEN=0.0.0.0 in $CONFIG_DIR/nabiman.env (plain HTTP)."
fi
echo "Login with the configured admin password."
