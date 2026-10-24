# One change, start to finish: the loop

Every change to a Bench goes through these steps, in this order. Doing them every time is what
makes the person's "1px off" complaints stop; skipping one is how a change ships unchecked.
Paths: the project is usually `~/Projects/<crate>` (made by `new-bench.sh` or `new-app.sh`;
anywhere else works the same); the kit's scripts are `~/Projects/bench-kit/.agents/skills/bench-kit/scripts/` (`K=`
below). Run every command from the project folder. Units: the scripts that end in `.py` take
**points** (as in the code), except `measure.py`, which takes **pixels** (pt × 2).

```bash
K=~/Projects/bench-kit/.agents/skills/bench-kit/scripts
cd ~/Projects/<crate>
```

## 1. Turn the request into a target you can measure

Write one line for yourself before touching code, with numbers where the request has none:
- "The composer's border is 1px higher than New session" → *the composer's top rule and the
  bottom button's top edge at the same y*.
- "Paddings a bit bigger" → *24 → 28 on every side of the question, the answer and the composer
  text* (one 4pt step, `build-workflow.md`).
- "Move Tools up next to Tasks" → *Tools then Tasks in the top strip, in every mode; nothing
  left in the status bar but New window and Settings*.
- "The card closes by itself" → *the card is still open 10 s after New task*.
Which area and which procedure: `ui-areas.md` (put / move / fit).

## 2. Find the code

- Grep a unique visible string: a label, a tooltip, a placeholder:
  `grep -rn '"Message Hermes' src/`, `grep -rn 'Tools  ⌘3' src/`.
- Find the same piece in the reference Bench, `~/Projects/bench-kit/src` (same paths). If the
  person's code differs from the reference, the reference is usually the fix
  (`build-workflow.md`).
- Read the whole function before editing. The value you need often has a comment saying why
  (`// 22 + the 2px selection stripe = the 24 of the headers`).

## 3. Measure BEFORE

Take a shot of **your own copy**. Never click, type or take a shot in the person's window; their
app keeps running untouched.

```bash
cargo build --release 2>&1 | grep -E '^(warning|error)|Finished'
$K/shot.sh target/release/<crate> /tmp/before.png 8          # prints the size in px and pt
```
- The copy opens where the person left off: their last session or mode. To show another state
  (a mode, a list, a dialog, a new task), add a temporary hook (step 5) first.
- The data sits behind a macOS permission (Reminders, Calendar, the microphone)? A bare binary
  is refused. Bundle and shoot the app instead: `scripts/bundle-macos.sh`, then
  `$K/shot.sh dist/<Title>.app /tmp/before.png 8 DEBUG_X=1`. The script starts a separate copy
  with `open -n` and leaves the person's copy alone.
- Wait longer when the app connects over SSH: 25–40 s.

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
  rules (the top strip: 0–35, the search band: 36–70, the composer: 737–772, the status bar:
  773–800 at the default size); inside the list or the chat, look at a `crop.py` picture of the
  area and read y off it (crop coordinates are the window's, in pt).
- `spans.py <png> <y0> <y1> [x0 x1]`: each icon, word or button along the band, its position,
  and the gaps between them and to both ends — the tool for "is the icon 16 from the edge", "is
  the label centered", "did Tools move".
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
  the composer stays the header's 36`.
- Use exact-string edits (an Edit tool, or a script that asserts the old text occurs exactly
  once). After `cargo fmt` the text changes: re-read before the next scripted edit.
- Change every place that shares the value (`ui-areas.md`, "When a change makes something else
  not fit"), and stale comments and docs that state the old value.

## 5. A temporary hook to reach the state (when needed)

One line, marked, at the end of the app's `new()` (or where the data arrives):
```rust
if std::env::var("DEBUG_UPCOMING").is_ok() { app.toggle_tasks(window, cx); app.select_task_list(List::Upcoming, window, cx); } // DEBUG-TMP
```
Call the same methods the buttons call: never post clicks.

**Where the hook goes matters.** At the end of `new()` it runs before anything has loaded; data
that arrives later (the sessions list, a chat's history, Reminders) **overwrites** what the hook
did. Then put it where that data arrives (right after `self.sessions = rows;`), or for fake
content at the top of the view's render, guarded so it runs once:
```rust
// top of render_chat: one fake answer with a code block and a changed file, to measure cards
if std::env::var("DEBUG_CHAT").is_ok() && self.turns.is_empty() { self.turns.push(Turn { user: "test".into(), answer: "Run:\n```bash\nls -la\n```\n".into(), ..Default::default() }); } // DEBUG-TMP
```
(`Turn` has `Default`; add what the card needs — a Changed files card: `files:
vec![FileEdit { path: "/tmp/x.rs".into(), diff: None }]`; both in `src/model.rs`.)

Hooks that worked:
- open a mode or a list;
- start a new session;
- open a session by title (in the handler that receives the sessions list);
- press New task and shoot twice, 10 s apart (for bugs that need time).

## 6. Build

```bash
cargo build --release 2>&1 | grep -E '^(warning|error)' -A7 | head -40
```
- Zero warnings, not "it built". An unused import is a leftover of the old code; delete it.
- The first build of a new project takes minutes. Run it in the background or with a long
  timeout; later builds take 20–30 s.
- An error you don't understand: read the whole message (the `-A7` lines). Then look up the
  API in the reference Bench (`grep -rn 'fn_name' ~/Projects/bench-kit/src`) or the gpui-kit
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

Also shoot what the change could have touched: the other modes (shared bars and strips), a long
list, the dark theme if colors changed.

## 8. Clean up

```bash
grep -rl 'DEBUG-TMP' src | xargs sed -i '' '/DEBUG-TMP/d'   # remove every hook line
grep -rn 'DEBUG-TMP\|DEBUG_' src                              # must print nothing
cargo fmt && cargo build --release 2>&1 | grep -E '^(warning|error)'; cargo test 2>&1 | grep 'test result'
```
Hooks: none left. Warnings: none. Tests: all pass. Delete the test files you made outside
`/tmp` and the scratch folders.

## 9. Ship it to the person

```bash
git add -A && git commit -m "<what changed, in the person's words>"   # a build is one commit
$K/ship.sh            # builds dist/Bench.<commit>.app and opens it next to the running one
```
**Builds are a carousel, one app per commit** (Bench is developed from inside Bench): commit,
then `scripts/bundle-macos.sh` makes `dist/Bench.<commit>.app` (7 characters of the hash) as a
NEW app next to the others, and `ship.sh` opens it beside the running one. **Never quit, kill or
replace the app the person runs** — it is the window they work in and talk to you through. They
try the new build and move the Dock icon to it when it's good; the newest 6 builds stay for
rolling back. Uncommitted changes stop the build (`BENCH_DIRTY=1` for a throwaway one).
The person tests the bundle, not `target/`. "I see no changes" happened when only the dev build
was rebuilt. Tell them the app's name (`Bench.3f2a91c.app`) so they know which one to try.

## 10. Report

- What changed, in the person's words ("the composer's rule and the button's edge are now one
  line").
- The numbers: before → after ("1454 vs 1456 px → both 1472").
- What you couldn't check yourself (anything that needs a click, sound, another machine) as
  "not checked".
- Commit only when the person is happy, or asks for it.

## The loop, as a checklist

1. target in numbers
2. find the code + the reference
3. shot + numbers before
4. one exact edit
5. hook if needed
6. build, zero warnings
7. shot + numbers after = target; other modes
8. hooks out, fmt, build, test
9. `ship.sh`
10. report with numbers
