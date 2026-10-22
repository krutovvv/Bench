#!/bin/bash
# Build "Bench.app" (release binary + icon + Info.plist) into dist/.
# The name shown is Bench; the bundle id stays local.hermes-gpui (macOS grants — local network,
# microphone — and the state folder belong to it), and so does the binary's name.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release

APP="dist/Bench.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/hermes-gpui "$APP/Contents/MacOS/hermes-gpui"

# Icon: render the SVG mark, then build an .icns with every size macOS asks for.
WORK=$(mktemp -d)
qlmanage -t -s 1024 -o "$WORK" assets/logo.svg >/dev/null 2>&1
# Quick Look paints the background white: cut the corners outside the rounded square back to
# transparent (ImageMagick's own SVG renderer would lose the gradient).
SRC="$WORK/logo.png"
magick "$WORK/logo.svg.png" \( -size 1024x1024 xc:none -fill white \
  -draw "roundrectangle 100,100,923,923,185,185" \) -compose CopyOpacity -composite "$SRC"
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
  <key>CFBundleName</key><string>Bench</string>
  <key>CFBundleDisplayName</key><string>Bench</string>
  <key>CFBundleIdentifier</key><string>local.hermes-gpui</string>
  <key>CFBundleExecutable</key><string>hermes-gpui</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Dictating messages to Hermes</string>
  <key>NSLocalNetworkUsageDescription</key><string>Connecting to Hermes gateways on your local network over SSH</string>
</dict>
</plist>
PLIST

# A stable identity, so macOS privacy grants (microphone, local network) belong to the app
# rather than to one particular build.
codesign --force --sign - --identifier local.hermes-gpui "$APP"

echo "Built: $APP"
