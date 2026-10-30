# One change, start to finish: the loop

Every change to a Bench goes through these steps, in this order. Doing them every time is what
makes the person's "1px off" complaints stop; skipping one is how a change ships unchecked.
Paths: the project is `~/Projects/bench` (crate `bench`; anywhere else works the same);
the kit's scripts are `.agents/skills/bench-kit/scripts/` (`K=` below). Run every command from the project folder. Units: the scripts that end in `.py` take
**points** (as in the code), except `measure.py`, which takes **pixels** (pt × 2).

```bash
cd ~/Projects/bench
K=.agents/skills/bench-kit/scripts
```

## 1. Turn the request into a target you can measure

Write one line for yourself before touching code, with numbers where the request has none:
- "The status bar's rule is 1px off under the sidebar" → *the status bar's top rule at the same
  y across the sidebar and the thread*.
- "Paddings a bit bigger" → *24 → 28 on every side of the question, the answer and the composer
  text* (one 4pt step, `build-workflow.md`).
- "Put the folder button next to +" → *Open folder then + in the top strip, on every sidebar;
  the strip's last icon still 16 from the divider*.
- "The card closes by itself" → *the card is still open 10 s after it opened*.
Which area and which procedure: `ui-areas.md` (put / move / fit).

## 2. Find the code

- Grep a unique visible string: a label, a tooltip, a placeholder:
  `grep -rn 'Hide sidebar  ⌘' src/`.
- Or open the row for it in the skill `bench-map`.
- Read the whole function before editing. The value you need often has a comment saying why
  (`// 22 + the 2px selection stripe = the 24 of the headers`).

## 3. Measure BEFORE

Take a shot of **your own copy**. Never click, type or take a shot in the person's window; their
app keeps running untouched.

```bash
cargo build 2>&1 | grep -E '^(warning|error)|Finished'
$K/shot.sh target/debug/bench /tmp/before.png 8          # prints the size in px and pt
```
- Pick the state first: `BENCH_DEMO=…` (made-up threads: `1`, `landing`, `top`, `long`, `new`,
  `server`, `tool`, `env`, `explorer`…) or `BENCH_HOME=<a copy of ~/.bench>` (real data, never the
  person's own folder): `$K/shot.sh target/debug/bench /tmp/before.png 8 BENCH_DEMO=landing`. Every
  demo state and hook: the table in `build-verification.md`. Neither reaches the state: add a hook
  (step 5).
- The data sits behind a macOS permission (the microphone, notifications)? A bare binary is
  refused, and Bench is one app: commit, ask the person to restart, and check it with them in Bench.
- Wait longer when the app connects over SSH: 25–40 s.

Look at it: read the shot itself (the read tool shrinks a 2480×1600 shot to fit and says by
how much), then `crop.py` the area at 2× for detail. Numbers come from the scripts below, never
from ImageMagick one-liners written on the spot: those cost one agent 14 steps of wrong crops.

Then read the numbers:
```bash
python3 -I $K/rules.py /tmp/before.png 100 600          # rules and edges down x=100 and x=600 pt
python3 -I $K/spans.py /tmp/before.png 8 28 0 280       # what is drawn along a band: icons, gaps (pt)
python3 -I $K/textx.py /tmp/before.png 280 40 60 95 110 # where text starts in two bands, from x=280
python3 -I $K/crop.py /tmp/before.png 260 720 420 80 /tmp/area.png 2   # look at one area, 2× zoom
python3 -I $K/measure.py /tmp/before.png row 1500 560 2480  # raw color runs, in PIXELS (pt × 2)
```
- `rules.py` x values: one column in the sidebar (around 100) and one in the main area (around
  600). The rules that must be one line across the window must show the same y in both.
- **Choosing the band's y**: run `rules.py` first — a bar's content lies between two of its
  rules (the title bar: 0–40, the status bar: the last 29 at the default size; the composer is a
  rounded field, not a band); inside the list or the chat, look at a `crop.py` picture of the
  area and read y off it (crop coordinates are the window's, in pt).
- `spans.py <png> <y0> <y1> [x0 x1]`: each icon, word or button along the band, its position,
  and the gaps between them and to both ends — the tool for "is the icon 16 from the edge", "is
  the label centered", "did the + move".
- `textx.py`'s first number is the divider's x (the sidebar's width, 280 by default). Each pair
  after it is a band (y from, y to) in pt.
- Look at crops of the area, never at the whole screen: other windows get into a full-screen shot.

- **Cards in the chat or the body** (a code block, Changed files): put the `rules.py` column
  inside the cards (x ≈ 600 on a 1240 window), then crop the same area and match each printed
  y to what you see — the output has no labels; a card's top border, its header rule and its
  bottom border all show as rules. A card's header height = its top border → its header rule.

Write the before numbers down: they go in the report. **Two complaints that need two states**
(the composer is visible on any screen; a code card only in a chat with one) → a before-shot per
state, each with its own hook.

## 4. Edit: one change, exact

- The smallest edit that meets the target, in the house style of the code around it (same
  helpers, same comment density). A comment says *why* a number is what it is, e.g. `// 26 so
  the plus and the mic match Send`.
- Use exact-string edits (an Edit tool, or a script that asserts the old text occurs exactly
  once). After `cargo fmt` the text changes: re-read before the next scripted edit.
- Change every place that shares the value (`ui-areas.md`, "When a change makes something else
  not fit"), and stale comments and docs that state the old value.

## 5. A hook to reach the state (when needed)

First try `BENCH_DEMO=…` / `BENCH_HOME=<copy>` (step 3). A state they lack is added for good:
a field with its doc comment in `src/demo/hooks.rs`, what it does in `src/demo/script.rs` (which
calls the same methods the buttons call, once the fake agent's answers are in), a line in the table
in `build-verification.md`. Never a branch in product code.

Fake content in the thread is a demo thread: a `Journal` in `src/demo/journal.rs` (`j.turn(user,
thought, calls, answer, at, secs)` with `tool("bash", "ls -la")` or `edit(path, diff)` calls), listed
in `src/demo/threads.rs`. A project's made-up environment is `src/demo/env.rs`; what the fake agent
answers an RPC or a REST call is `src/demo/agent.rs`.

## 6. Build

```bash
cargo build 2>&1 | grep -E '^(warning|error)' -A7 | head -40
```
- Zero warnings, not "it built". An unused import is a leftover of the old code; delete it.
- The first build of a new project takes minutes. Run it in the background or with a long
  timeout; later builds take 20–30 s.
- An error you don't understand: read the whole message (the `-A7` lines). Then look up how
  the code already calls it (`grep -rn 'fn_name' src crates`) or the gpui-kit
  pages (the skill `gpui-kit`, `references/usage.md`). Don't guess method names: `ui-gotchas.md` lists those that
  already went wrong.

## 7. Measure AFTER, compare with the target

The same shot and the same commands as in step 3 (with the hook's variable if you added one).
Compare with the target from step 1:
- Rules that must meet: the same y (to the half point).
- Insets that must be equal: within 0.5pt.
- Heights that must match: the same rule-to-rule distance.
- "Doesn't close by itself": a second shot after the wait shows the same state.
Off? Back to step 4 with the measured difference, not a guess.

Also shoot what the change could have touched: threads with other inserts (shared heads and strips), a long
list, the dark theme if colors changed.

## 8. Clean up

```bash
cargo fmt && cargo build 2>&1 | grep -E '^(warning|error)'; cargo test --workspace 2>&1 | grep 'test result'
```
`cargo fmt` changes only your files: the tree is kept formatted. If it ever touches a file you
didn't change, that file was committed unformatted: leave it in its own commit, not yours.
Warnings: none. Tests: all pass. Delete the test files you made outside `/tmp` and the scratch folders.

## 9. Commit, build the app, ask for a restart

The steps are `SKILL.md`'s ("Working on Bench from inside Bench"). `git add` only the paths you changed.

**A grant that is on but doesn't work** (local network above all: the toggle is on in Privacy &
Security, macOS never asks, and every request to a local service fails).
macOS matched the app to a dead entry: an old path, or another app with the same Mach-O UUID.
Its local network list can't be cleaned. `tccutil` doesn't cover it, and
`/Library/Preferences/com.apple.networkextension.plist` can't be edited on a running Mac (the
only reset is deleting it from Recovery, which also wipes every VPN). Don't fight it. **Give the
app a new bundle id** (`ID=` in `scripts/bundle-macos.sh`, then fix the id in this skill and in
`src/main.rs`): macOS sees a new app and asks for every grant again (network, microphone,
notifications). The person confirms them once, and it works. That is how the release
went from `local.bench` to `local.bench.app` (2026-10-25). `bundle-macos.sh` also gives each app
its own Mach-O UUID. Keep that.
The person tests the bundle, not `target/`. "I see no changes" happened when only `target/`
was rebuilt. Build the app, then ask them to restart Bench and try it.

## 10. Report

- What changed, in the person's words ("the composer's rule and the button's edge are now one
  line").
- The numbers: before → after ("1454 vs 1456 px → both 1472").
- What you couldn't check yourself (anything that needs a click, sound, another machine) as
  "not checked".

## The loop, as a checklist

1. target in numbers
2. find the code (the skill `bench-map`)
3. shot + numbers before
4. one exact edit
5. a demo hook if needed
6. build, zero warnings
7. shot + numbers after = target; other threads
8. fmt, build, test
9. commit, `scripts/bundle-macos.sh`, then ask the person to restart Bench
10. report with numbers
