#!/bin/bash
# Build Bench (release binary + icon + Info.plist) at its one place: /Applications/Bench.app,
# bundle id local.bench.app. There is one app: the person works in it, and ship.sh rebuilds it
# in place when they ask. (/System/Applications is macOS's own read-only volume.)
#
# It keeps one path and one bundle id and is signed with the Apple Development certificate from
# the keychain: macOS then knows Bench by team and bundle id, not by the build's hash, so privacy
# grants (microphone, local network, notifications) are given once and survive rebuilds. Local
# network privacy also tells apps apart by the executable's Mach-O UUID, derived below from the
# linker's and the bundle id (it stays the same, or the grant is asked again). The id was
# local.bench until macOS's local network list filled with dead local.bench entries it matched
# instead of asking; there is no way to remove them, so the app took a fresh id. Without such a
# certificate the build falls back to ad-hoc signing (grants are asked again after every build);
# BENCH_SIGN=<identity> picks another one.
#
# It quits nothing (ship.sh does): a running Bench keeps running from its old (deleted) copy and
# the new one starts on its next open. Nothing is parked beside it: rolling back means
# rebuilding an earlier commit, so uncommitted changes stop a build.
set -euo pipefail
cd "$(dirname "$0")/.."

HASH=$(git rev-parse --short=7 HEAD)
# `.agents/` (the agents' skills) never goes into the app, so skills being written there don't
# stop a build.
if [[ -n "$(git status --porcelain --untracked-files=normal -- . ':!dist' ':!.agents')" ]]; then
    echo "Uncommitted changes — commit them first (a build is one commit's):" >&2
    git status --short -- . ':!dist' | head -20 >&2
    exit 1
fi

NAME="Bench"
ID=local.bench.app
APP="/Applications/Bench.app"
COUNT=$(git rev-list --count HEAD)

SIGN=${BENCH_SIGN:-$(security find-identity -v -p codesigning |
    awk -F'"' '/"Apple Development:/ { split($1, a, " "); print a[2]; exit }')}

cargo build --release

# Assembled next to the target, then moved into place in one step.
mkdir -p "$(dirname "$APP")"
NEW="$(dirname "$APP")/.$NAME.new.app"
rm -rf "$NEW"
mkdir -p "$NEW/Contents/MacOS" "$NEW/Contents/Resources"
# Named as the app: macOS titles the menu bar by the process, so "bench" read lowercase there.
cp target/release/bench "$NEW/Contents/MacOS/Bench"
# Its own Mach-O UUID per app (from the linker's one and the bundle id), before signing.
python3 -I - "$NEW/Contents/MacOS/Bench" "$ID" <<'PY'
import hashlib, struct, sys
path, ident = sys.argv[1], sys.argv[2]
b = bytearray(open(path, "rb").read())
assert struct.unpack_from("<I", b, 0)[0] == 0xFEEDFACF, "not a thin 64-bit Mach-O"
off = 32
for _ in range(struct.unpack_from("<I", b, 16)[0]):
    cmd, size = struct.unpack_from("<II", b, off)
    if cmd == 0x1B:  # LC_UUID
        b[off + 8 : off + 24] = hashlib.md5(bytes(b[off + 8 : off + 24]) + ident.encode()).digest()
        break
    off += size
else:
    sys.exit("no LC_UUID")
open(path, "wb").write(b)
PY
# bench-agent's sources (the Pi backend); Bench installs them into ~/.bench/app and runs
# `npm ci` there, so node_modules stays out of every build.
mkdir -p "$NEW/Contents/Resources/agent"
cp -R agent/src agent/scripts agent/package.json agent/package-lock.json "$NEW/Contents/Resources/agent/"

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
iconutil -c icns "$ICONSET" -o "$NEW/Contents/Resources/AppIcon.icns"
rm -rf "$WORK"

cat > "$NEW/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>$ID</string>
  <key>CFBundleExecutable</key><string>Bench</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.$COUNT</string>
  <key>CFBundleVersion</key><string>$COUNT</string>
  <key>BenchCommit</key><string>$HASH</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Dictating messages to the agent</string>
  <key>NSLocalNetworkUsageDescription</key><string>Connecting to the agent on machines on your local network over SSH</string>
</dict>
</plist>
PLIST

if [[ -n "$SIGN" ]]; then
    codesign --force --sign "$SIGN" --identifier "$ID" "$NEW"
else
    echo "no Apple Development certificate: signing ad-hoc (privacy grants won't survive rebuilds)" >&2
    codesign --force --sign - --identifier "$ID" "$NEW"
fi

if pgrep -f "$APP/Contents/MacOS/" >/dev/null; then
    echo "a Bench from $APP is still running — the new one starts on its next open" >&2
fi
rm -rf "$APP"
mv "$NEW" "$APP"

echo "Built: $APP ($HASH)"
