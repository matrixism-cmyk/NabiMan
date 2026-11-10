#!/usr/bin/env bash
# Fails when a Dockerfile pulls a base image by tag alone. Every external FROM must
# carry @sha256:<digest> (see the comment at the top of the Dockerfile for how to
# resolve one). Stage references (FROM <earlier stage>) and `scratch` are fine.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
status=0
while IFS= read -r file; do
    stages=" scratch "
    while IFS= read -r line; do
        # FROM [--platform=...] image [AS name]
        set -- $line
        shift
        while [ $# -gt 0 ] && [ "${1#--}" != "$1" ]; do shift; done
        image=${1:-}
        if [ $# -ge 3 ] && [ "${2,,}" = "as" ]; then stages+="$3 "; fi
        case "$stages" in *" $image "*) continue ;; esac
        if [[ "$image" != *@sha256:* ]]; then
            echo "$file: base image not pinned by digest: $image" >&2
            status=1
        fi
    done < <(grep -iE '^[[:space:]]*FROM[[:space:]]' "$file")
done < <(git ls-files -- 'Dockerfile' '*/Dockerfile' '*.Dockerfile' 'Dockerfile.*')
[ "$status" -eq 0 ] && echo "All Dockerfile base images are pinned by digest."
exit "$status"
