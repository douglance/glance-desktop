#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP="$ROOT/target/Glance.app/Contents/MacOS/Glance"
if [ ! -x "$APP" ]; then
    printf 'Build Glance with scripts/bundle.sh first.\n' >&2
    exit 1
fi
exec "$APP" --mcp
