# Verifying UI changes

The user judges spacing to the pixel. "Looks fine to me" is not verification. Measure.

## Never in the user's window

The user is usually working in the running app. Do not click, type or send synthetic events into
it, and do not overwrite its saved state (last place, prefs) for a test. Start your own dev
instance and look at *its* window only.

## Capture your own window

The step-by-step loop with every command is `build-change-loop.md`; this page is the why and
the edge cases.

`scripts/shot.sh <binary | App.app> <out.png> [seconds] [KEY=VALUE …]`: launches your own copy
(a binary directly; an .app with `open -n` so it has the person's permissions), waits, finds
the largest on-screen window of that PID (`scripts/winid.swift` via CoreGraphics), captures it
with `screencapture -x -o -l <id>` (no shadow) and quits the copy. Matching by process name is
wrong — the user's copy has the same name; match by PID. Then `scripts/rules.py` (rules and
edges in pt down a column), `scripts/textx.py` (where text starts), `scripts/crop.py` (one area,
zoomed).

- Wait long enough for the backend: local ~12 s, SSH targets 25–40 s.
- The dev instance restores the user's last place (backend/profile/session). To show a specific
  state, use a temporary env hook instead of editing the user's state files. One line at the end
  of the app's `new()`, marked so it can't be forgotten, removed before the final build:
  ```rust
  if std::env::var("DEBUG_UPCOMING").is_ok() { app.toggle_tasks(window, cx); app.select_task_list(List::Upcoming, window, cx); } // DEBUG-TMP
  ```
  or in the handler where the data arrives (`self.sessions = rows;` → open the session whose
  title contains `$DEBUG_OPEN`). Afterwards: `sed -i '' '/DEBUG-TMP/d' src/app.rs` and
  `grep -c DEBUG_ src/app.rs` must print 0. Hooks that worked: open a mode/list, open a session
  by title, start a new session, press "New task" and screenshot twice 10 s apart (a bug that
  needs time to appear).
- **Data behind a macOS permission** (Reminders, Calendar, Contacts, the microphone): the grant
  belongs to the .app, so a binary run from a terminal is refused. Bundle, then launch YOUR copy
  as the app: `open -n dist/<App>.app --env DEBUG_X=1`, find it with
  `pgrep -nf "<App>.app/Contents/MacOS"` (the newest), capture by that PID, `kill` it after.
- States that need a click (menus, popovers, hover) can't be captured this way — say so in the
  report and ask the user to check, don't claim them as verified. If you must click, do it in
  YOUR dev instance only: get its window frame (`osascript -e 'tell application "System Events"
  to get {position, size} of window 1 of (first process whose unix id is <PID>)'`) and post
  CGEvent mouse events at screen points from a small Swift script; the first click may only
  activate the window. Never in the user's window or their live session.
- Content that needs data the backend doesn't have (a media-rich answer): inject a fake item
  behind a temporary env var (`APP_MEDIA_DEMO=<dir>`), screenshot, then delete the hook. If the
  dev instance must open a different place, back up the app's state files first and restore
  them afterwards.
- Logic without UI (a video decoder's timing, a parser) — a temporary `#[ignore]` test run with
  `--ignored --nocapture`, deleted after.

### Synthetic input: only into a window you are sure is frontmost

Posting CGEvents (clicks, keys) to test a dev instance is risky: activation can fail or another
window can sit over the click point, and the events then land in the **user's** app (typed
text, ⌘P = Print…). That happened once. Prefer a temporary hook that calls the app's methods
(open, type into a field via `set_value`, press via the method the button calls). If you must
post events, check right before each one that your window is frontmost and owns the point, and
stop at the first doubt; tell the user if anything may have gone astray. Note: GPUI inputs do
not emit `InputEvent::Change` for `set_value`, so "dirty" logic driven by Change needs one real
keystroke to verify.

## Measure

Screenshots are Retina: 2 px = 1 pt. `scripts/measure.py` prints runs of equal color along a
row or column:

```
python3 -I measure.py shot.png column <x> <y0> <y1>   # e.g. down through the sidebar
python3 -I measure.py shot.png row <y> <x0> <x1>      # e.g. across the composer
```

Typical checks that settled real complaints:
- **Every bar = 36 and the rules on one row**: down a sidebar column and down a main-area column
  the rule rows must be identical — header bottom = search band top (px 70–71), search band
  bottom = tag strip bottom, bottom button top = composer rule (e.g. both at 1472).
- The rule above the composer and the top edge of the sidebar's bottom button: same y.
- Text start in the composer vs. the button's distance from the divider (glyph side bearing
  makes text look 1pt further — fix the exact number of px measured).
- Left/right gaps around a centered thing (waveform) are equal.
- Text starts across areas (question band, answer, composer) share one x — scan each row from
  the divider for the first dark pixel; glyphs differ by ~0.5pt, more is a real offset.
- Two cards' header lines (code block vs Changed files): rule-to-rule distance equal.
- A row of things that can collide (a segmented control in a fixed-width card): nothing touches
  the edge.

Find elements by their exact colors (button fill, sidebar `#EFEFF0`, border `#D9D9DA`) with
numpy rather than guessing coordinates; crops of a full-screen shot catch other windows.

## Before saying "done"

- `cargo build` with zero warnings, `cargo test`, `cargo fmt`.
- Protocol/API behaviour confirmed with the headless `--probe` mode, read-only where the data is
  the user's (create-and-delete only throwaway items, and say so).
- Bundle rebuilt, commit made.
- `hermes verify` auto-detect assumes an HTTP server (polls a port after `start`) and reports
  `ok:false` for a GUI app. Write `.hermes/environment.json` with the build/test commands and
  `"start": null` — the readiness phase is then skipped and a passing run records the evidence.
- Report: what was measured/seen, what only compiles, what needs the user's click.

## Editing code safely

- After `cargo fmt` the text differs from what a script expects: re-read before the next
  scripted edit; prefer exact single edits.
- Never write a file from a buffer read before another edit of the same file — it silently
  reverted a feature once (the `@` prompt list). If a script does several edits, either apply
  them all to one buffer or re-read between them.
