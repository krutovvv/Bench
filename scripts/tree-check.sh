#!/bin/zsh
# The rules that keep this code easy for a small model to work in (skill bench-structure), as a check.
# Prints every break and exits 1: what it lists over 500 lines is the next thing to cut.
cd "$(dirname "$0")/.."
failed=0
say() { print -r -- "$1"; failed=1 }

for file in src/*.rs(N); do
  case "$file" in src/main.rs|src/app.rs) ;; *) say "$file: move it into a folder named for its place or its feature" ;; esac
done

for file in src/**/*.rs crates/*/src/**/*.rs; do
  lines=$(wc -l < "$file")
  [ "$lines" -le 500 ] || say "$file: $lines lines, cut it by what its parts do (limit about 500)"
  head -1 "$file" | grep -q '^//!' || say "$file: its first line must be a //! comment saying what it holds"
done

for name in utils helpers misc common; do
  for file in src/**/$name.rs(N) crates/*/src/**/$name.rs(N); do say "$file: name the file for its job"; done
done

# A path a document names must exist: a stale document misleads a weak model more than a missing one.
# Not a variable called `path`: in zsh that is tied to PATH.
for doc in AGENTS.md SPEC.md .agents/skills/bench-*/SKILL.md .agents/skills/app-playbook/SKILL.md; do
  grep -oE '`(src|crates|scripts|assets|web|agent)/[A-Za-z0-9_./-]+`' "$doc" | tr -d '`' | sort -u | while read -r named; do
    [ -e "$named" ] || say "$doc names $named, which does not exist"
  done
done

exit "$failed"
