#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/target/Pachiri.app/Contents/MacOS/Pachiri"
if [ ! -x "$APP" ]; then
    printf 'Build Pachiri with scripts/bundle.sh first.\n' >&2
    exit 1
fi
exec "$APP" --mcp
