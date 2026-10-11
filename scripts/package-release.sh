#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "Usage: $0 BACKEND_BINARY FRONTEND_BUILD OUTPUT_DIR" >&2
    exit 2
fi

binary=$(realpath "$1")
helper=$(dirname "$binary")/nabiman-privileged-helper
recovery=$(dirname "$binary")/nabiman-recovery
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
frontend=$(realpath "$2")
output=$3
arch=$(uname -m)
case "$arch" in
    x86_64|aarch64) ;;
    *) echo "Unsupported release architecture: $arch" >&2; exit 1 ;;
esac

if [ ! -x "$binary" ] || [ ! -x "$helper" ] || [ ! -x "$recovery" ] || [ ! -f "$frontend/index.html" ]; then
    echo "All three backend binaries and frontend index.html are required" >&2
    exit 1
fi

python3 "$repo_root/scripts/release-inventory.py" generate --source "$repo_root" --binary "$binary" --static "$frontend"

mkdir -p "$output"
output=$(cd "$output" && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

mkdir -p "$stage/nabiman/static"
cp "$binary" "$stage/nabiman/nabiman-server"
cp "$helper" "$stage/nabiman/nabiman-privileged-helper"
install -m 755 "$recovery" "$stage/nabiman/nabiman-recovery"
install -m 755 "$repo_root/scripts/management-recovery.py" "$stage/nabiman/management-recovery.py"
chmod 755 "$stage/nabiman/nabiman-server" "$stage/nabiman/nabiman-privileged-helper"
cp -a "$repo_root/packaging" "$stage/nabiman/packaging"
cp -a "$frontend/." "$stage/nabiman/static/"

install -m 755 "$repo_root/scripts/verify-release.py" "$stage/nabiman/verify-release.py"
python3 - "$stage/nabiman" <<'PYMANIFEST'
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1]);files={}
for p in sorted(root.rglob('*')):
    if p.is_symlink():raise SystemExit('Package symlinks are forbidden')
    if p.is_file():files['nabiman/'+p.relative_to(root).as_posix()]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'size':p.stat().st_size,'mode':p.stat().st_mode&0o777}
(root/'release-manifest.json').write_text(json.dumps({'schema_version':1,'kind':'unsigned-package-file-manifest','files':files},indent=2)+'\n')
PYMANIFEST
name="nabiman-linux-${arch}.tar.gz"
tar -C "$stage" -czf "$output/$name" nabiman
(
    cd "$output"
    sha256sum "$name" > "$name.sha256"
)
python3 "$repo_root/scripts/verify-release.py" "$output/$name"
cp "$stage/nabiman/static/release-bom.cdx.json" "$output/$name.cdx.json"
cp "$stage/nabiman/release-manifest.json" "$output/$name.manifest.json"
cp "$stage/nabiman/static/release-inventory.json" "$output/$name.inventory.json"
cp "$repo_root/scripts/verify-release.py" "$output/verify-release.py"
echo "Created verified $output/$name with lockfile inventory and manifest"
