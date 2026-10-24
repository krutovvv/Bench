#!/bin/bash
# The browser demo: Bench compiled to WebAssembly, always in demo mode (src/demo.rs), into
# web/dist/ — a static folder for GitHub Pages or any web server.
#
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli --version <the
# wasm-bindgen version in Cargo.lock>. Serve it to try: python3 -m http.server -d web/dist 8765
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release --target wasm32-unknown-unknown
OUT=web/dist
rm -rf "$OUT"
mkdir -p "$OUT/assets"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" target/wasm32-unknown-unknown/release/bench.wasm
# A new build must not be served from the browser's cache: the page asks for this build's files.
STAMP="$(git rev-parse --short=7 HEAD)-$(date +%s)"
sed "s/__BUILD__/$STAMP/g" web/index.html > "$OUT/index.html"
cp assets/logo.svg "$OUT/assets/"
# gpui-kit's own icons (menus, checkmarks) are fetched next to the page in the browser.
KIT_ASSETS=$(cargo metadata --format-version 1 | python3 -c 'import json,sys; print(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"]=="gpui-kit-assets").rsplit("/",1)[0])')
cp -R "$KIT_ASSETS/assets/icons" "$OUT/assets/"
cp -R assets/demo "$OUT/assets/"
touch "$OUT/.nojekyll"
du -sh "$OUT"
echo "Built: $OUT — try it: python3 -m http.server -d $OUT 8765"
