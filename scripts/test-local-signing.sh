#!/bin/sh
# Verify that two different signed bundle versions satisfy the same requirement.
set -eu
APP="${1:-target/Glance.app}"
TASK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/glance-signing-qa.XXXXXX")"
trap 'rm -rf "$TASK_DIR"' EXIT
trap 'exit 1' HUP INT TERM
ditto "$APP" "$TASK_DIR/first.app"
./scripts/sign-app.sh "$TASK_DIR/first.app"
ditto "$TASK_DIR/first.app" "$TASK_DIR/second.app"
/usr/libexec/PlistBuddy -c 'Set :CFBundleVersion 2' "$TASK_DIR/second.app/Contents/Info.plist"
./scripts/sign-app.sh "$TASK_DIR/second.app"
FIRST_REQUIREMENT="$(codesign -d -r- "$TASK_DIR/first.app" 2>&1 | sed -n 's/^#* *designated => //p')"
SECOND_REQUIREMENT="$(codesign -d -r- "$TASK_DIR/second.app" 2>&1 | sed -n 's/^#* *designated => //p')"
FIRST_HASH="$(codesign -d --verbose=4 "$TASK_DIR/first.app" 2>&1 | sed -n 's/^CDHash=//p')"
SECOND_HASH="$(codesign -d --verbose=4 "$TASK_DIR/second.app" 2>&1 | sed -n 's/^CDHash=//p')"
test -n "$FIRST_REQUIREMENT"
test "$FIRST_REQUIREMENT" = "$SECOND_REQUIREMENT"
test -n "$FIRST_HASH"
test "$FIRST_HASH" != "$SECOND_HASH"
codesign --verify --strict -R "=$FIRST_REQUIREMENT" "$TASK_DIR/second.app"
printf 'PASS: different bundle hashes satisfy the same certificate-based requirement.\n'
