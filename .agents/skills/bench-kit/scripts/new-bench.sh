#!/bin/bash
# A new person's Bench: a copy of the reference Bench (the Hermes chat, settings, providers,
# SSH terminals, mini IDE, Tools), renamed so it lives beside other apps. The name in the
# window and the Dock stays "Bench", the icon stays Bench's: every Bench looks the same
# outside and differs inside. Then remove the modes the person doesn't need
# (references/bench-modes-remove.md) and add theirs (references/build-modes.md).
# Usage: new-bench.sh [crate-name] [dest-dir]
#   crate-name  lowercase, dashes; the binary, the data folder and the bundle id local.<crate>
#               (default: bench)
#   dest-dir    default ~/Projects/<crate-name>
set -euo pipefail
CRATE=${1:-bench}
DEST=${2:-$HOME/Projects/$CRATE}
HERE=$(cd "$(dirname "$0")/.." && pwd)
# The kit lives in the reference Bench's repository (<bench>/.agents/skills/bench-kit): the new
# Bench is a copy of that repository's committed files — its latest working version, kit included.
BENCH=$(cd "$HERE/../../.." && pwd)

[[ "$CRATE" =~ ^[a-z][a-z0-9-]*$ ]] || { echo "crate name: lowercase letters, digits, dashes" >&2; exit 1; }
[[ -e "$DEST" ]] && { echo "$DEST already exists" >&2; exit 1; }

mkdir -p "$DEST"
git -C "$BENCH" ls-files -z | (cd "$BENCH" && xargs -0 tar -cf -) | tar -xf - -C "$DEST"
rm -f "$DEST/dist/.build-number"
# The reference's own name → this Bench's crate name (Cargo, bundle id, data and cache folders).
# The reference's own identity → this Bench's crate name. Only the exact identifiers ("bench" is
# also a word in names and comments): the crate, the binary, the bundle id, the state folders.
grep -rlE 'name = "bench"|local\.bench|(Support|Caches)/bench|release/bench|MacOS/bench|<string>bench</string>|bak-bench|config\.bench' \
    "$DEST" --include='*.rs' --include='*.toml' --include='*.sh' --include='*.lock' --include='*.md' |
    while read -r f; do
        sed -i '' -E -e "s/name = \"bench\"/name = \"$CRATE\"/" -e "s/local\.bench/local.$CRATE/g" \
            -e "s#(Support|Caches)/bench#\1/$CRATE#g" -e "s#release/bench#release/$CRATE#g" \
            -e "s#MacOS/bench#MacOS/$CRATE#g" -e "s#<string>bench</string>#<string>$CRATE</string>#g" \
            -e "s/bak-bench/bak-$CRATE/g" -e "s/config\.bench\./config.$CRATE./g" "$f"
    done
chmod +x "$DEST"/scripts/*.sh

# One shared build folder for all such apps: gpui compiles once.
SHARED="$HOME/Library/Caches/gpui-apps/target"
mkdir -p "$SHARED" "$DEST/.cargo"
printf '[build]\ntarget-dir = "%s"\n' "$SHARED" > "$DEST/.cargo/config.toml"
ln -s "$SHARED" "$DEST/target"
grep -q '^/target' "$DEST/.gitignore" 2>/dev/null || printf '/target\n/dist\n' >> "$DEST/.gitignore"

git -C "$DEST" init -q && git -C "$DEST" add -A && git -C "$DEST" commit -qm "Bench from bench-kit" || true
echo "Created $DEST"
echo "Next: cd $DEST && cargo build && ./target/debug/$CRATE   (bundle: scripts/bundle-macos.sh)"
