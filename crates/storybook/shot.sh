#!/bin/bash
# The storybook's two reference pictures, light and dark, into crates/storybook/reference/.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -p storybook
SHOT=.agents/skills/bench-kit/scripts/shot.sh
mkdir -p crates/storybook/reference
"$SHOT" target/debug/storybook crates/storybook/reference/light.png 6
"$SHOT" target/debug/storybook crates/storybook/reference/dark.png 6 STORYBOOK_DARK=1
