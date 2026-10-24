#!/bin/bash
# A plain Bench: the frame (sidebar, header, status bar, settings, theme, kit.rs) without the
# Hermes chat — for an app the person asked to have no agent in (e.g. "just a text editor").
# The usual start is scripts/new-bench.sh (with the chat). Same look, same Bench icon.
# Usage: new-app.sh <crate-name> "<Title>" [dest-dir]
#   crate-name   lowercase, dashes: also the binary, the state folder and local.<crate> bundle id
#   Title        what the window, the menu bar and the .app are called
#   dest-dir     default ~/Projects/<crate-name>
set -euo pipefail
CRATE=${1:?crate name}
TITLE=${2:?title}
DEST=${3:-$HOME/Projects/$CRATE}
HERE=$(cd "$(dirname "$0")/.." && pwd)
TPL="$HERE/templates/app"

[[ "$CRATE" =~ ^[a-z][a-z0-9-]*$ ]] || { echo "crate name: lowercase letters, digits, dashes" >&2; exit 1; }
[[ -e "$DEST" ]] && { echo "$DEST already exists" >&2; exit 1; }

cp -R "$TPL" "$DEST"
find "$DEST" -type f \( -name '*.rs' -o -name '*.toml' -o -name '*.sh' -o -name '*.md' \) -print0 |
  xargs -0 sed -i '' -e "s|__CRATE__|$CRATE|g" -e "s|__TITLE__|$TITLE|g"
chmod +x "$DEST/scripts/bundle-macos.sh"

# The icon: Bench's, the same for every app this kit makes.
# The kit lives in the reference Bench's repository: <bench>/.agents/skills/bench-kit.
BENCH=$(cd "$HERE/../../.." && pwd)
cp "$BENCH/assets/logo.svg" "$DEST/assets/logo.svg"

# One build folder for every app made from this template: gpui and the other dependencies
# compile once (the first build of the first app takes minutes, later apps start from there).
# ./target is a link to it, so ./target/debug/<crate> and the bundle script work as usual.
SHARED="$HOME/Library/Caches/gpui-apps/target"
mkdir -p "$SHARED" "$DEST/.cargo"
printf '[build]\ntarget-dir = "%s"\n' "$SHARED" > "$DEST/.cargo/config.toml"
ln -s "$SHARED" "$DEST/target"

git -C "$DEST" init -q
echo "Created $DEST"
echo "Next: cd $DEST && cargo build && ./target/debug/$CRATE   (bundle: scripts/bundle-macos.sh)"
