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
#   -h, --help

REPO="${NABIMAN_REPO:-matrixism-cmyk/NabiMan}"
INSTALL_DIR="/usr/local/bin"
STATIC_DIR="/usr/local/share/nabiman/static"
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

usage() {
    cat <<'USAGE'
Usage: sudo bash install.sh [--version vX.Y.Z] [--base-url URL]
                            [--archive FILE [--signature FILE]]
                            [--insecure-skip-verify]
USAGE
}
while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION=${2:?--version needs a value}; shift 2 ;;
        --base-url) BASE_URL=${2:?--base-url needs a value}; shift 2 ;;
        --archive) LOCAL_ARCHIVE=${2:?--archive needs a file}; shift 2 ;;
        --signature) LOCAL_SIGNATURE=${2:?--signature needs a file}; shift 2 ;;
        --insecure-skip-verify) SKIP_VERIFY=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1 (see --help)" >&2; exit 2 ;;
    esac
done

echo "=== NabiMan Installer ==="

# Check root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: Please run as root (sudo)"
    exit 1
fi

# Check dependencies
for cmd in curl tar mktemp python3; do
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

ASSET="nabiman-linux-$ARCH.tar.gz"
if [ -z "$BASE_URL" ]; then
    if [ "$VERSION" = "latest" ]; then
        BASE_URL="https://github.com/$REPO/releases/latest/download"
    else
        BASE_URL="https://github.com/$REPO/releases/download/$VERSION"
    fi
fi
STAGE=$(mktemp -d)
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
