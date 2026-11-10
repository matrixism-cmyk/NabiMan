#!/usr/bin/env bash
# Lists (and with --apply, removes) old ad-hoc deployments on a development box
# laid out like /workspace/nabiman:
#
#   ROOT/static                 -> ROOT/deployments/<name>/static   (live web UI)
#   ROOT/deployments/<name>/    one directory per deploy
#   ROOT/bin/nabiman-server     live binary; ROOT/bin/*.bak.* older copies
#   ROOT/static.bak.*           web UI copies from before deployments/
#
# Customer installs do not use this: nabiman-update keeps its own last N versions.
#
# Usage: prune-deployments.sh [--root DIR] [--keep N] [--dry-run | --apply]
#   --keep N   newest N of each kind to keep (default 5)
#   --dry-run  only list what would go (the default)
#   --apply    delete it (needs the owner's approval on a shared box)
#
# Never touched, whatever --keep says: the deployment ROOT/static points into,
# the one its static.previous points to (the one-step rollback), anything a
# running nabiman process executes from or has as its working directory, and
# files in bin/ that are not *.bak.* copies.
set -euo pipefail

ROOT=/workspace/nabiman
KEEP=5
APPLY=0
while [ $# -gt 0 ]; do
    case "$1" in
        --root) ROOT=${2:?--root needs a directory}; shift 2 ;;
        --keep) KEEP=${2:?--keep needs a number}; shift 2 ;;
        --dry-run) APPLY=0; shift ;;
        --apply) APPLY=1; shift ;;
        -h|--help) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done
case "$KEEP" in ''|*[!0-9]*) echo "--keep must be a number" >&2; exit 2 ;; esac
ROOT=$(cd "$ROOT" && pwd -P)

declare -A PROTECT=()
protect() { local p; p=$(readlink -f -- "$1" 2>/dev/null) || return 0; PROTECT["$p"]=1; }
# The deployment (or top-level directory) a path lives in.
owner_of() {
    local p=$1
    case "$p" in
        "$ROOT"/deployments/*) p=${p#"$ROOT"/deployments/}; echo "$ROOT/deployments/${p%%/*}" ;;
        "$ROOT"/bin/*) echo "$p" ;;
        "$ROOT"/*) p=${p#"$ROOT"/}; echo "$ROOT/${p%%/*}" ;;
    esac
}

if [ -L "$ROOT/static" ]; then
    live=$(readlink -f "$ROOT/static")
    protect "$(owner_of "$live")"
    live_dir=$(dirname "$live")
    if [ -L "$live_dir/static.previous" ]; then protect "$(owner_of "$(readlink -f "$live_dir/static.previous")")"; fi
fi
running=()
for proc in /proc/[0-9]*; do
    # Another user's exe link is unreadable; its command line is not.
    exe=$(readlink "$proc/exe" 2>/dev/null) || exe=$({ tr '\0' '\n' < "$proc/cmdline"; } 2>/dev/null | head -n1) || continue
    exe=${exe% (deleted)}
    [ -n "$exe" ] || continue
    case "${exe##*/}" in nabiman-*) ;; *) continue ;; esac
    running+=("$exe")
    protect "$(owner_of "$exe")"
    cwd=$(readlink "$proc/cwd" 2>/dev/null) || cwd=""
    if [ -n "$cwd" ]; then protect "$(owner_of "$cwd")"; fi
done

size_of() { du -sb -- "$1" 2>/dev/null | awk '{print $1}'; }
human() { numfmt --to=iec --suffix=B "$1" 2>/dev/null || echo "${1}B"; }

total=0; count=0; failed=0
declare -A KIND_TOTAL=() KIND_COUNT=()
newest_first() { find "$@" -printf '%T@ %p\n' | sort -rn | cut -d' ' -f2-; }
consider() { # kind, newest-first candidate list on stdin (not a pipe: totals must survive)
    local kind=$1 kept=0 path size note
    while IFS= read -r path; do
        [ -n "$path" ] || continue
        if [ -n "${PROTECT[$(readlink -f -- "$path")]:-}" ]; then
            printf 'keep   %-9s %s (live or running)\n' "$kind" "${path#"$ROOT"/}"
            continue
        fi
        if [ "$kept" -lt "$KEEP" ]; then
            kept=$((kept + 1)); printf 'keep   %-9s %s (newest %d)\n' "$kind" "${path#"$ROOT"/}" "$KEEP"
            continue
        fi
        size=$(size_of "$path"); size=${size:-0}
        note=""
        # Deploy directories sometimes hold copies of auth data taken before a cutover.
        if [ -d "$path" ] && { [ -e "$path/cutover.auth" ] || [ -e "$path/data.before" ] || [ -e "$path/users.migrated.json" ]; }; then
            note=" [holds a pre-cutover data/auth copy]"
        fi
        printf 'remove %-9s %-60s %8s%s\n' "$kind" "${path#"$ROOT"/}" "$(human "$size")" "$note"
        total=$((total + size)); count=$((count + 1))
        KIND_TOTAL[$kind]=$(( ${KIND_TOTAL[$kind]:-0} + size )); KIND_COUNT[$kind]=$(( ${KIND_COUNT[$kind]:-0} + 1 ))
        if [ "$APPLY" -eq 1 ]; then rm -rf -- "$path" || failed=$((failed + 1)); fi
    done
}

echo "Root: $ROOT   keep newest $KEEP of each kind   mode: $([ "$APPLY" -eq 1 ] && echo APPLY || echo dry-run)"
if [ ${#running[@]} -gt 0 ]; then printf 'Running: %s\n' "${running[@]}"; fi
if [ -L "$ROOT/static" ]; then echo "Live static: $(readlink "$ROOT/static")"; fi
echo
if [ -d "$ROOT/deployments" ]; then
    consider deploy < <(newest_first "$ROOT/deployments" -mindepth 1 -maxdepth 1 -type d)
fi
if [ -d "$ROOT/bin" ]; then
    consider bin-bak < <(newest_first "$ROOT/bin" -mindepth 1 -maxdepth 1 -type f -name '*.bak.*')
fi
consider static-bak < <(newest_first "$ROOT" -mindepth 1 -maxdepth 1 -type d -name 'static.bak.*')

echo
for kind in "${!KIND_COUNT[@]}"; do printf '%-10s %4d entries %8s\n' "$kind" "${KIND_COUNT[$kind]}" "$(human "${KIND_TOTAL[$kind]}")"; done
if [ "$APPLY" -eq 1 ]; then
    echo "Removed $count entries, $(human "$total")$([ "$failed" -gt 0 ] && echo ", $failed failed (permissions?)")."
else
    echo "Would remove $count entries, $(human "$total"). Nothing was deleted; run with --apply to delete."
fi
