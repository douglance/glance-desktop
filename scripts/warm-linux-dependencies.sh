#!/bin/sh
# Compile the application's dependencies without its source or final LTO work.
set -eu
cd "$(dirname "$0")/.."
[ "$(uname -s)" = Linux ] || { echo 'Run this script on Linux.' >&2; exit 1; }

# Cargo uses the same features and release profile as the real binary. The
# temporary package keeps the checked-out source intact. A distinct binary name
# also leaves any real release executable in the target directory untouched.
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/glance-dependencies.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
trap 'exit 1' HUP INT TERM
cp Cargo.toml Cargo.lock "$STAGE/"
cat >> "$STAGE/Cargo.toml" <<'TOML'

[[bin]]
name = "glance-dependency-cache"
path = "src/main.rs"
TOML
mkdir -p "$STAGE/src"
printf 'fn main() {}\n' > "$STAGE/src/main.rs"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(pwd)/target}"
cargo build --release --locked --bin glance-dependency-cache --manifest-path "$STAGE/Cargo.toml" --timings
# Only the compiled libraries belong in the reusable cache.
rm -f "$CARGO_TARGET_DIR/release/glance-dependency-cache" \
    "$CARGO_TARGET_DIR/release/glance-dependency-cache.d" \
    "$CARGO_TARGET_DIR/release/deps/"glance_dependency_cache-*
