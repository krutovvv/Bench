# Verifying UI changes

**The main way to check a change is a picture of your own copy:** `cargo build`, then
`.agents/skills/bench-kit/scripts/shot.sh target/debug/bench /tmp/after.png 10 BENCH_DEMO=…`, then
read the picture (and measure). It works from inside Bench too, once Bench is allowed in Screen
Recording. A change the person will see is not done until you have looked at it this way: before
you commit, before you build the app, before you report.

The user judges spacing to the pixel. "Looks fine to me" is not verification. Measure.

## Never in the user's window

The user is usually working in the running app. Do not click, type or send synthetic events into
it, and do not overwrite its saved state (last place, prefs) for a test. Start your own copy and
look at *its* window only.

## Capture your own window

The step-by-step loop with every command is `build-change-loop.md`; this page is the why, the demo's
hooks and the edge cases.

`.agents/skills/bench-kit/scripts/shot.sh <binary | App.app> <out.png> [seconds] [KEY=VALUE …]`:
launches your own copy (a binary directly; an .app with `open -n` so it has the person's
permissions), waits, finds the largest on-screen window of that PID (`scripts/winid.swift` via
CoreGraphics), captures it with `screencapture -x -o -l <id>` (no shadow) and quits the copy.
Matching by process name is wrong: the user's copy has the same name; match by PID. Then
`scripts/rules.py` (rules and edges in pt down a column), `scripts/textx.py` (where text starts),
`scripts/crop.py` (one area, zoomed).

- Wait long enough: the demo ~8 s, a real agent on this Mac ~12 s, SSH targets 25–40 s.
- **Every demo state against a reference:** `.agents/skills/bench-kit/scripts/pixels/` shoots all of
  `states.txt` and compares them with a reference taken on the commit before (its `README.md`). Run
  it on any change that touches many screens: a refactor keeps pixels identical.
- A state the hooks below don't reach is a hook added for good: a field in `src/demo/hooks.rs` with
  its doc comment, what it does in `src/demo/script.rs`, a line in the table here. Data the demo lacks
  is a fixture in `src/demo/env.rs`, `threads.rs` or `journal.rs`.
- **Data behind a macOS permission** (the microphone, notifications): the grant belongs to the .app,
  so a binary run from a terminal is refused, and Bench is one app. Commit, ask the person to
  restart, check it with them in Bench.
- States that need a click (a menu, a hover): `BENCH_DEMO_CLICK=<x>,<y>` presses inside your copy.
  What no hook can reach: say so in the report and ask the user to check, don't claim it as verified.
- Logic without UI (a video decoder's timing, a parser): a test.

## The demo: a fake agent

`BENCH_DEMO=…` runs the whole app against a fake agent, as Zed's `FakeFs`: nothing is connected and
nothing of the person's is shown. Product code never asks whether it is the demo. The seam is
`src/client/fake.rs` (`FakeAgent`, installed once by `use_fake`): `Client`, `Connection` and the few
readers of the machine ask it first. `src/demo/agent.rs` answers RPC and REST from the fixtures in
`env.rs`, `threads.rs`, `journal.rs`; `main.rs` calls `demo::install()` before the first window and
`demo::attach()` on each window; `src/demo/hooks.rs` (`DemoHooks`) reads the variables below once at
start; `src/demo/script.rs` does what they ask of the window from outside, as the person would have;
`src/demo/probes.rs` gives it real input (a drop, a click). The demo's state goes to a temporary
folder of its own.

This table is the one list of hooks; `src/demo/hooks.rs` has each with its doc comment. A hook works
with `BENCH_DEMO` set.

| Variable | Value | What the window shows |
|---|---|---|
| `BENCH_DEMO` | `1`, `landing`, `top`, `long`, `server`, `tool`, `new`, `new-project`, `explorer`, `env` | What it opens on: the browser thread, a thread at its end, at its top, 60 exchanges, a server's thread, the tools insert, a new thread, the new project's page, the blog in the Explorer, the blog's Settings |
| `BENCH_EXPLORER` | `settings`, `part`, `lsp`, `lsp-new`, `diff`, `terminal`, `problems`, `thread`, `specs`, `comment` | In the Explorer: that document or panel |
| `BENCH_ENV` | `telegram`, `model` | Its Settings page scrolled there |
| `BENCH_DEMO_SIDE` | `threads` | The tree's place given to the project's threads |
| `BENCH_DEMO_TOOL` | `html`, `preview`, `long` | The tools insert's pane that scrolls inside the thread; `long` a page past the card's cap |
| `BENCH_DEMO_JOURNAL` | a file | A saved `thread.open` answer drawn in the landing's place |
| `BENCH_DEMO_WORK` | `<n>[:<i>]` | The n-th work open, its i-th call's card open |
| `BENCH_DEMO_WORK_CALLS` | `<c>` | Of the works with c calls |
| `BENCH_DEMO_WORK_DOWN` | `<px>` | That much further down into the opened work |
| `BENCH_DEMO_CARD` | `0` | The milestone open, not its card |
| `BENCH_DEMO_SCROLL` | `end`, `<item>:<px>` | The thread scrolled there |
| `BENCH_DEMO_FILES` | `1`, `diff` | The changed files unfolded; `diff` the first one's diff under its row |
| `BENCH_DEMO_PROCS` | `1` | The background processes' card unfolded with its output |
| `BENCH_DEMO_REVIEW` | `1` | The turn reviewed, its notes unfolded |
| `BENCH_DEMO_ARM` | `<key>` | That delete waiting for its second press (`thread:d-api`, `p:p-web`) |
| `BENCH_DEMO_JUMP` | `1` | "Jump to latest" pressed two seconds in |
| `BENCH_DEMO_PROJECT` | `1`, `env`, `long` | The new project's page, with an exported environment or a long folder |
| `BENCH_DEMO_NEW` | `1` | The dock panel's "+" pressed two seconds in |
| `BENCH_DEMO_TYPE` | `1`, words | Typed into the composer two seconds in |
| `BENCH_DEMO_SEND` | `1` | Send pressed three seconds in |
| `BENCH_DEMO_RUN` | `1` | A run plays in the open thread |
| `BENCH_DEMO_PAUSED` | `1` | The run holds mid-work |
| `BENCH_CLARIFY` | `1`, `answered` | The agent's question over the thread |
| `BENCH_CLARIFY_MULTI` | `1` | It takes several answers |
| `BENCH_DEMO_NOMODEL` | `1` | No main model set, as a fresh install |
| `BENCH_PANEL` | `providers` | The gear open |
| `BENCH_GEAR_TAB` | `0`, `1`, `2` | On that tab |
| `BENCH_DEMO_QUEUE` | `1` | Two messages and a plan waiting |
| `BENCH_DEMO_DRAFT` | words | These words in the composer |
| `BENCH_PREFILL` | words | These words in the composer, from the first frame |
| `BENCH_DEMO_BESIDE` | `<thread id>` (`d-tool`) | That thread beside the open one |
| `BENCH_DEMO_SEARCH` | `1`, words | ⌘K open, with those words typed |
| `BENCH_DEMO_RENAME` | `<thread id>` | That thread being renamed |
| `BENCH_DEMO_DROPLINE` | `<group>:above`, `<group>:below` | The sidebar's drop line over a project's group |
| `BENCH_DEMO_POP` | `agent` | The agent's popover open, with long lines in its log |
| `BENCH_DEMO_DROP` | `chat`, `between`, `right`, `foot`, `side`, `tint` | A file carried into the window there and dropped; `tint` held there, undropped |
| `BENCH_DEMO_CLICK` | `<x>,<y>` | The left button pressed and let go there, two seconds in |
| `BENCH_TOOLS_SHOWN` | `1` | The composer's hover tools shown at rest |

Besides the demo: `BENCH_HOME=<a copy of ~/.bench>` runs on a copy of real data (its agent is started
by hand, `BENCH_HOME=… node agent/src/server.ts`; never the person's own folder), and
`BENCH_TRACE_SCROLL=1` logs, every frame, where the thread's content stands.

### Synthetic input: only into a window you are sure is frontmost

Posting CGEvents (clicks, keys) to test a dev instance is risky: activation can fail or another
window can sit over the click point, and the events then land in the **user's** app (typed
text, ⌘P = Print…). That happened once. Prefer a demo hook that calls the app's methods
(open, type into a field via `set_value`, press via the method the button calls), or
`BENCH_DEMO_CLICK`, whose input stays inside your copy. If you must
post events, check right before each one that your window is frontmost and owns the point, and
stop at the first doubt; tell the user if anything may have gone astray. Note: GPUI inputs do
not emit `InputEvent::Change` for `set_value`, so "dirty" logic driven by Change needs one real
keystroke to verify.

## Measure

Screenshots are Retina: 2 px = 1 pt. `.agents/skills/bench-kit/scripts/measure.py` prints runs of equal color along a
row or column:

```
python3 -I measure.py shot.png column <x> <y0> <y1>   # e.g. down through the sidebar
python3 -I measure.py shot.png row <y> <x0> <x1>      # e.g. across the composer
```

Typical checks that settled real complaints:
- **Bars and their rules on one row**: the title bar's rule (y 40 pt) crosses the window in one
  pixel row; in the IDE the Explorer's bar and the documents' tabs end on one rule; the sidebar's
  foot and the status bar's rule line up with the main area's.
- Text starts within a column share one x: a group head and its rows (`SIDEBAR_TEXT_X`), the
  composer's text and its chips; glyphs differ by ~0.5pt, more is a real offset.
- Left/right gaps around a centered thing (waveform) are equal.
- Two cards' header lines (code block vs Changed files): rule-to-rule distance equal.
- A row of things that can collide (a segmented control in a fixed-width card): nothing touches
  the edge.

Find elements by their exact colors (the sidebar `#F6F6F7`, a hairline `#E2E3E7`, a selected row `#E6E7EB`) with
numpy rather than guessing coordinates; crops of a full-screen shot catch other windows.

## Before saying "done"

- `cargo build` with zero warnings, `cargo test`, `cargo fmt`.
- Protocol/API behaviour confirmed with the headless `--probe` mode, read-only where the data is
  the user's (create-and-delete only throwaway items, and say so).
- Commit made, the app built, a restart asked for: `SKILL.md` has the loop.
- Report: what was measured/seen, what only compiles, what needs the user's click.

## Editing code safely

- After `cargo fmt` the text differs from what a script expects: re-read before the next
  scripted edit; prefer exact single edits.
- Never write a file from a buffer read before another edit of the same file — it silently
  reverted a feature once (the `@` prompt list). If a script does several edits, either apply
  them all to one buffer or re-read between them.
