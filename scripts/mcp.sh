#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
if [ "$(uname -s)" = Linux ]; then
    APP="${GLANCE_EXECUTABLE:-$ROOT/target/release/glance}"
else
    APP="${GLANCE_EXECUTABLE:-$ROOT/target/Glance.app/Contents/MacOS/Glance}"
fi
if [ ! -x "$APP" ]; then
    printf 'Build with scripts/bundle.sh (macOS) or scripts/package-linux.sh (Linux) first.\n' >&2
    exit 1
fi
exec "$APP" --mcp
