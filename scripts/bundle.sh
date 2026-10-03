#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
MODE="${1:-release}"
if [ "$MODE" = "release" ]; then cargo build --release --locked; else cargo build --locked; fi
APP="$(pwd)/target/Pachiri.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cargo run --release --locked --example icon -- target/Pachiri.iconset
# A new resource name prevents macOS from reusing an earlier design's icon cache.
ICON_HASH="$(shasum -a 256 target/Pachiri.iconset/icon_512x512@2x.png | cut -c 1-12)"
ICON_NAME="Pachiri-$ICON_HASH.icns"
iconutil -c icns target/Pachiri.iconset -o "$APP/Contents/Resources/$ICON_NAME"
/usr/bin/swiftc -target "$(uname -m)-apple-macosx12.0" -O native/video_encoder.swift -o target/pachiri-video-encoder
cp target/pachiri-video-encoder "$APP/Contents/MacOS/pachiri-video-encoder"
cp assets/gpui/LICENSE-APACHE "$APP/Contents/Resources/GPUI-LICENSE"
cp assets/lucide/LICENSE "$APP/Contents/Resources/Lucide-LICENSE"
cp "target/$MODE/pachiri" "$APP/Contents/MacOS/Pachiri"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>Pachiri</string>
<key>CFBundleIdentifier</key><string>dev.benv.pachiri</string>
<key>CFBundleIconFile</key><string>Pachiri.icns</string>
<key>CFBundleName</key><string>Pachiri</string>
<key>CFBundleDisplayName</key><string>Pachiri</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSScreenCaptureUsageDescription</key><string>Pachiri captures your selected screen area for annotation.</string>
</dict></plist>
PLIST
/usr/libexec/PlistBuddy -c "Set :CFBundleIconFile $ICON_NAME" "$APP/Contents/Info.plist"
codesign --force --deep --sign - "$APP"
printf 'Built %s\n' "$APP"
