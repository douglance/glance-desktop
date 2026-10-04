#!/bin/sh
# Exercise CLI capture cancellation/failure without a desktop or real capture.
set -eu
APP="${1:?Usage: scripts/test-linux-platform.sh /absolute/path/to/glance}"
MOCK="$(mktemp -d)"
trap 'rm -rf "$MOCK"' EXIT
trap 'exit 1' HUP INT TERM
cat > "$MOCK/slurp" <<'SELECT'
#!/bin/sh
exit 1
SELECT
cat > "$MOCK/grim" <<'CAPTURE'
#!/bin/sh
echo 'mock compositor capture failure' >&2
exit 1
CAPTURE
chmod +x "$MOCK/slurp" "$MOCK/grim"
# Cancellation must exit successfully before GPUI tries to open a window.
PATH="$MOCK:$PATH" WAYLAND_DISPLAY=glance-test "$APP" --capture-area
if PATH="$MOCK:$PATH" WAYLAND_DISPLAY=glance-test "$APP" --capture-screen 2> "$MOCK/error"; then
    echo 'Expected capture failure' >&2
    exit 1
fi
grep -q 'mock compositor capture failure' "$MOCK/error"
printf 'CLI capture cancellation and error reporting passed\n'
