#!/bin/sh
# Explicit one-time user trust setup. Scope is code signing, never SSL/TLS.
# The certificate/private key remain in the Glance-specific keychain.
set -eu
SIGN_DIR="${GLANCE_SIGNING_DIR:-$HOME/Library/Application Support/Glance/Signing}"
test -f "$SIGN_DIR/certificate.pem"
test -f "$SIGN_DIR/local-signing.keychain-db"
security add-trusted-cert -r trustRoot -p codeSign \
    -k "$SIGN_DIR/local-signing.keychain-db" "$SIGN_DIR/certificate.pem"
printf 'Glance development certificate trusted for code signing in the user domain.\n'
