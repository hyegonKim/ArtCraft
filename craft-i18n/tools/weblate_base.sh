#!/bin/sh
# weblate_base.sh <weblate-ref> <out-dir>: the base for tools/weblate_bridge.py, `main`'s
# translations as of the last export on <weblate-ref>, rendered as PO. Empty before the first export.
set -eu
ref=$1 out=$2
EXPORT_NAME=${EXPORT_NAME:-craft-i18n export}
sha=$(git log "$ref" --author="$EXPORT_NAME" -1 --format=%s | sed -n 's/^Export from main@\([0-9a-f]\{40\}\)$/\1/p')
if [ -z "$sha" ]; then echo "no export on $ref yet: empty base" >&2; exit 0; fi
tree=$(mktemp -d)
git archive "$sha" locales tools/plural_forms.tsv | tar -x -C "$tree"
rmdir "$out"
python3 "$(dirname "$0")/weblate_bridge.py" export "$out" --base "$tree/none" --root "$tree"
