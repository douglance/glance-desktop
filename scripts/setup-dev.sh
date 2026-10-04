#!/bin/sh
# Install this checkout's optional formatting/lint hook without changing global Git settings.
set -eu
cd "$(dirname "$0")/.."
git config --local core.hooksPath .githooks
printf 'Enabled Glance pre-commit checks (cargo fmt and cargo clippy).\n'
