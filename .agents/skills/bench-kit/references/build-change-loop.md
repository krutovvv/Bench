# One change, start to finish: the loop

Every change to a Bench goes through these steps, in this order. Doing them every time is what
makes the person's "1px off" complaints stop; skipping one is how a change ships unchecked.
Paths: the project is `~/Projects/bench` (crate `bench`; anywhere else works the same);
the kit's scripts are `~/Projects/bench/.agents/skills/bench-kit/scripts/` (`K=`
below). Run every command from the project folder. Units: the scripts that end in `.py` take
**points** (as in the code), except `measure.py`, which takes **pixels** (pt × 2).

```bash
K=~/Projects/bench/.agents/skills/bench-kit/scripts
cd ~/Projects/bench
```

## 1. Turn the request into a target you can measure

Write one line for yourself before touching code, with numbers where the request has none:
- "The status bar's rule is 1px off under the sidebar" → *the status bar's top rule at the same
  y across the sidebar and the thread*.
- "Paddings a bit bigger" → *24 → 28 on every side of the question, the answer and the composer
  text* (one 4pt step, `build-workflow.md`).
- "Put the folder button next to +" → *Open folder then + in the top strip, on every sidebar;
  the strip's last icon still 16 from the divider*.
- "The card closes by itself" → *the card is still open 10 s after New task*.
Which area and which procedure: `ui-areas.md` (put / move / fit).

## 2. Find the code

- Grep a unique visible string: a label, a tooltip, a placeholder:
  `grep -rn '"Search sessions' src/`, `grep -rn 'Hide sidebar  ⌘' src/`.
- Or open the row for it in `docs/map.md`.
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
  `tasks`, `server`, `tool`, `env`, `explorer`…) or `BENCH_HOME=<a copy of ~/.bench>` (real
  data, never the person's own folder): `$K/shot.sh target/debug/bench /tmp/before.png 8
  BENCH_DEMO=landing`. Every demo state and hook: `build-verification.md`. Only when neither
  reaches the state, a temporary hook (step 5).
- The data sits behind a macOS permission (the microphone, notifications)? A bare binary is
  refused, and Bench is one app: commit, ask the person to ship, and check it with them in Bench.
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

## 5. A temporary hook to reach the state (when needed)

First try `BENCH_DEMO=…` / `BENCH_HOME=<copy>` (step 3); a state they lack is better added to
`src/demo/mod.rs` for good. Otherwise one line, marked, at the end of the app's `new()` (or where the
data arrives):
```rust
if std::env::var("DEBUG_UPCOMING").is_ok() { app.open_task_list(List::Upcoming, window, cx); } // DEBUG-TMP
```
Call the same methods the buttons call: never post clicks.

**Where the hook goes matters.** At the end of `new()` it runs before anything has loaded; data
that arrives later (the sessions list, a chat's history, the tasks) **overwrites** what the hook
did. Then put it where that data arrives (right after `self.sessions = rows;`). Fake content in
the thread is a demo thread, not a hook: a `Journal` in `src/demo/mod.rs` (`j.turn(user, thought,
calls, answer, at, secs)` with `tool("bash", "ls -la")` or `edit(path, diff)` calls), shown with
`thread(app, id, title, subject, j)` under its own `BENCH_DEMO=<name>`.

Hooks that worked:
- open a thread with an insert in it (`open_subject_key` with the insert's key, e.g. `ssh:local`) or a list;
- start a new session;
- open a session by title (in the handler that receives the sessions list);
- press New task and shoot twice, 10 s apart (for bugs that need time).

## 6. Build

```bash
cargo build 2>&1 | grep -E '^(warning|error)' -A7 | head -40
```
- Zero warnings, not "it built". An unused import is a leftover of the old code; delete it.
- The first build of a new project takes minutes. Run it in the background or with a long
  timeout; later builds take 20–30 s.
- An error you don't understand: read the whole message (the `-A7` lines). Then look up how
  the code already calls it (`grep -rn 'fn_name' src crates`) or the gpui-kit
  pages (`gpui-kit-usage.md`). Don't guess method names: `ui-gotchas.md` lists those that
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
grep -rl 'DEBUG-TMP' src | xargs sed -i '' '/DEBUG-TMP/d'   # remove every hook line
grep -rn 'DEBUG-TMP\|DEBUG_' src                              # must print nothing
cargo fmt && cargo build 2>&1 | grep -E '^(warning|error)'; cargo test 2>&1 | grep 'test result'
```
`cargo fmt` changes only your files: the tree is kept formatted. If it ever touches a file you
didn't change, that file was committed unformatted: leave it in its own commit, not yours.
Hooks: none left. Warnings: none. Tests: all pass. Delete the test files you made outside
`/tmp` and the scratch folders.

## 9. Ship it to the person — only when they ask

Commit the change, then build the app at once — no need to ask — and ask the person to restart
Bench. As Bench's own agent never quit it yourself: your run would be cut off.

```bash
git add -A && git commit -m "<what changed, in the person's words>"   # a build is one commit
scripts/bundle-macos.sh   # rebuilds /Applications/Bench.app in place, quits nothing
# then: "собрано — перезапусти Bench"   (from outside Bench: $K/ship.sh quits, rebuilds, opens)
```
**One app** (Bench is developed from inside Bench): the person works in
**Bench** (`/Applications/Bench.app`, `local.bench.app`), rebuilt in place;
there is no other build. Bench is
a client: threads, the queue and the agent live in the agent process and `~/.bench`, so quitting
and reopening it costs the person a few seconds and an unfinished run comes back with the window.
Never kill it or quit it (only `ship.sh`, from outside). Rolling back means rebuilding an earlier commit —
nothing is parked beside `/Applications/Bench.app`. Signed with the Apple Development
certificate, so macOS grants are given once. Uncommitted changes stop the build.

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
its own Mach-O UUID, so Dev and release built from one commit are never twins. Keep that.
The person tests the bundle, not `target/`. "I see no changes" happened when only `target/`
was rebuilt. Build the app, then ask them to restart Bench and try it.

## 10. Report

- What changed, in the person's words ("the composer's rule and the button's edge are now one
  line").
- The numbers: before → after ("1454 vs 1456 px → both 1472").
- What you couldn't check yourself (anything that needs a click, sound, another machine) as
  "not checked".
- Commit only when the person is happy, or asks for it.

## The loop, as a checklist

1. target in numbers
2. find the code (`docs/map.md`)
3. shot + numbers before
4. one exact edit
5. hook if needed
6. build, zero warnings
7. shot + numbers after = target; other threads
8. hooks out, fmt, build, test
9. `scripts/bundle-macos.sh`, then ask the person to restart Bench
10. report with numbers
