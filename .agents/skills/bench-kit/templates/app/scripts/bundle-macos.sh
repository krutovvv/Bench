#!/bin/bash
# Build "__TITLE__.app" (release binary + icon + Info.plist) into dist/.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

APP="dist/__TITLE__.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/__CRATE__ "$APP/Contents/MacOS/__CRATE__"

# Icon: render the SVG mark, then build an .icns with every size macOS asks for.
WORK=$(mktemp -d)
qlmanage -t -s 1024 -o "$WORK" assets/logo.svg >/dev/null 2>&1
SRC="$WORK/logo.svg.png"
ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s "$SRC" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) "$SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$WORK"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>__TITLE__</string>
  <key>CFBundleDisplayName</key><string>__TITLE__</string>
  <key>CFBundleIdentifier</key><string>local.__CRATE__</string>
  <key>CFBundleExecutable</key><string>__CRATE__</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <!-- Add a usage string for every protected resource the app touches, e.g.
  <key>NSMicrophoneUsageDescription</key><string>…</string>
  <key>NSLocalNetworkUsageDescription</key><string>…</string> -->
</dict>
</plist>
PLIST

# A stable identity, so macOS privacy grants (microphone, local network) belong to the app
# rather than to one particular build.
codesign --force --sign - --identifier local.__CRATE__ "$APP"

echo "Built: $APP"
