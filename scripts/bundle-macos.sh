#!/bin/bash
# Build the current commit as its own app: "dist/Bench.<commit>.app" (release binary + icon +
# Info.plist), <commit> = the first 7 characters of the commit's hash, as in `git log --oneline`.
#
# Bench is developed from inside Bench, so a build never replaces or quits the app that runs:
# every commit gets a new app next to the others; the person tries it (open
# "dist/Bench.<commit>.app") and moves the Dock icon over when it's good. The last KEEP builds
# stay for rolling back; older ones are removed, but never one that is running, and never
# "Bench.app".
#
# Commit first: a build is one commit, so uncommitted changes stop it. For a quick experiment,
# BENCH_DIRTY=1 scripts/bundle-macos.sh builds "Bench.<commit>-dirty.app" anyway.
# All versions share the bundle id local.bench (macOS grants — Reminders, notifications,
# microphone, local network — and the state folder belong to it) and the binary's name.
set -euo pipefail
cd "$(dirname "$0")/.."
KEEP=6

HASH=$(git rev-parse --short=7 HEAD)
if [[ -n "$(git status --porcelain --untracked-files=normal -- . ':!dist')" ]]; then
    if [[ "${BENCH_DIRTY:-}" == 1 ]]; then
        HASH="$HASH-dirty"
    else
        echo "Uncommitted changes — commit them first (a build is one commit's), or BENCH_DIRTY=1:" >&2
        git status --short -- . ':!dist' | head -20 >&2
        exit 1
    fi
fi
APP="dist/Bench.$HASH.app"
if pgrep -f "$PWD/$APP/Contents/MacOS/" >/dev/null; then
    echo "$APP is running — it is this commit's build already; quit it to rebuild it" >&2
    exit 1
fi
COUNT=$(git rev-list --count HEAD)

cargo build --release

mkdir -p dist
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/bench "$APP/Contents/MacOS/bench"
# bench-agent's sources (the Pi backend); Bench installs them into ~/.bench-agent/app and runs
# `npm ci` there, so node_modules stays out of every build.
mkdir -p "$APP/Contents/Resources/agent"
cp -R agent/src agent/scripts agent/package.json agent/package-lock.json "$APP/Contents/Resources/agent/"

# Icon: render the SVG mark, then build an .icns with every size macOS asks for.
WORK=$(mktemp -d)
# Transparent corners, and the soft shadow under the tile that macOS icons carry (the merge
# grows the canvas by the shadow's reach; the crop puts the tile back on the 1024 grid).
SRC="$WORK/logo.png"
magick -background none -density 96 assets/logo.svg -resize 1024x1024 \
  \( +clone -background black -shadow 22x14+0+12 \) +swap -background none -layers merge \
  +repage -crop 1024x1024+28+16 +repage "$SRC"
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
  <key>CFBundleIdentifier</key><string>local.bench</string>
  <key>CFBundleExecutable</key><string>bench</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.$COUNT</string>
  <key>CFBundleVersion</key><string>$COUNT</string>
  <key>BenchCommit</key><string>$HASH</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Dictating messages to the agent</string>
  <key>NSLocalNetworkUsageDescription</key><string>Connecting to the agent on machines on your local network over SSH</string>
  <key>NSRemindersUsageDescription</key><string>Your tasks live in Reminders: Bench shows and edits them</string>
  <key>NSRemindersFullAccessUsageDescription</key><string>Your tasks live in Reminders: Bench shows and edits them</string>
</dict>
</plist>
PLIST

# A stable identity, so macOS privacy grants (microphone, local network) belong to the app
# rather than to one particular build.
codesign --force --sign - --identifier local.bench "$APP"

# Keep the newest KEEP builds (by date); skip any that is running, never touch "Bench.app".
OLD_BUILDS=$(ls -dt dist/Bench.*.app 2>/dev/null | tail -n +$((KEEP + 1)))
printf '%s\n' "$OLD_BUILDS" | while read -r old; do
    [[ -z "$old" ]] && continue
    if pgrep -f "$PWD/$old/Contents/MacOS/" >/dev/null; then
        echo "kept (running): $old"
    else
        rm -rf "$old" && echo "removed old build: $old"
    fi
done

echo "Built: $APP"
echo "Try it next to the running one: open \"$APP\""
