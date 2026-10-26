# Workflow in detail (do it in this order)

The loop for every single change, with the commands: `build-change-loop.md`.

## A change to how Bench looks or behaves

Most requests ("the border is 1px off", "move the icon", "more air", "the card closes by
itself") are about one value in one place. Before writing code:
1. Find the place: `docs/map.md`, or `grep -rn` the visible label ("Search", "Copy") or the
   helper `ui-catalog.md` names.
2. A size or a gap is a constant in `crates/ui` (`metrics.rs`, `Gap`): change it there, so every
   place that shares it follows; never a `px(…)` in a screen.
3. Measure before and after (`build-verification.md`); say which side was off and by how much.
"Proportionally smaller", "a little more" with no number: one step on the 4px grid (24 → 28),
the same on every side and in every place that shares it.

## The steps

1. **Read the code around the piece you build** before writing new code (`src/ui/*.rs` for the
   frame, `crates/ui/src/` for elements, `crates/ui/src/icons.rs`). Copy its idioms.
2. **Protocol work: probe it headless first.** `bench --probe [local|<ssh-host>]` (`src/main.rs`)
   connects without a window and prints what the agent has; verify protocols there, not by
   clicking.
3. **Build** with `cargo build` (dev profile opt-level 1, deps 2 — fast enough to run). Fix all
   warnings; `cargo fmt` before every commit.
4. **Verify visually by measuring**, not by eyeballing: launch your own copy (`BENCH_DEMO=…` or
   `BENCH_HOME=<a copy>`), capture *its* window, measure gaps (`build-verification.md`). Never
   click or type in the person's window.
5. **Commit every change; build the app only when the person asks** (usually once, after a
   batch): `ship.sh` (`build-change-loop.md`, step 9). **One app** (Bench is developed from
   inside Bench): the person works in `/Applications/Bench.app`; `ship.sh` quits it with
   `osascript … quit`, rebuilds it in place and opens it again. The person tests that bundle,
   not `target/` — "I see no changes" happened when only `target/` was rebuilt. Bench is a
   client: threads, the queue and the agent live in the agent process and `~/.bench`, so
   reopening it costs a few seconds and an unfinished run comes back with the window. Never kill
   it or quit it outside `ship.sh`. Rolling back means rebuilding an earlier commit.
6. **Report honestly** what was verified and what was only built (anything that needs a click).
