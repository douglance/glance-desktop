# Glance app icon

`glance.svg` is the production master. It uses glance.sh's existing white and
green pixel eye on a dark rounded tile, with transparent margins for macOS.
The source mark is from `modem-dev/agentpaste/app/icon.svg` at commit
`698ab598477e809f2f75ae624e63cbc6a20b0846`.

`examples/icon.rs` renders the SVG at 1024 pixels, then creates all ten standard
macOS iconset PNGs with premultiplied filtering to preserve alpha edges.
`scripts/bundle.sh` packages them into a hash-named Glance ICNS resource.

Prior artwork and its original generation prompts are preserved in `archive/`.
