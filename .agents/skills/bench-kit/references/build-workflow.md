# Workflow in detail (do it in this order)

The loop for every single change, with the commands: `build-change-loop.md`.

## A change to an existing Bench: look at the reference first

Most requests about a Bench's look or behaviour ("the border is 1px off", "move the icon",
"more air", "the card closes by itself") were already solved once in the reference Bench. Before
writing code:
1. Find the same piece in the reference Bench, `~/Projects/bench/src`:
   `grep -rn` the visible label ("Search sessions", "Hide sidebar", "Copy") or the helper the
   catalog names.
2. Compare the person's code with it. Different → bring theirs to the reference (that is the
   fix, with the reason on the page that describes it). The same → the request goes *further*
   than the reference: then change both values in one step and keep the kit's pages in line.
3. Measure before and after (`build-verification.md`); say which side was off and by how much.
"Proportionally smaller", "a little more" with no number: one step on the 4px grid (24 → 28,
26 → 22 circles with 11px icons), the same on every side and in every place that shares it.


1. **Read the reference app's code for the piece you build** before writing new code
   (`src/ui/*.rs` for views, `crates/ui/src/theme.rs`, `crates/ui/src/icons.rs`). Copy its idioms.
2. **Only for apps that talk to a server or an API: probe it headless first.** Give the binary a
   `--probe` mode that connects, calls the APIs and prints results; verify protocols there, not
   by clicking. An app that only shows local files needs no probe.
3. **Build** with `cargo build` (dev profile opt-level 1, deps 2 — fast enough to run). Fix all
   warnings; `cargo fmt` before every commit.
4. **Verify visually by measuring**, not by eyeballing: launch the dev build, capture *its*
   window by PID, measure gaps in pixels (`references/build-verification.md`). Never click or type in
   the user's live window; pre-fill state via env vars or a temporary hook instead.
5. **Commit every change; build the app only when the user asks** (usually once, after a batch):
   `ship.sh` (`build-change-loop.md`, step 9). The user tests the bundle, not `target/debug` —
   "I see no changes" happened when only the dev build was rebuilt.
   **Two apps, Dev and release** (Bench is developed from inside Bench): the person works in
   **Bench** (`/Applications/Bench.app`, `local.bench.app`). `ship.sh` quits it with
   `osascript … quit`, rebuilds it in place and opens it again.
(`dist/Bench Dev.app`, `local.bench.dev`) only to compare two states of the interface. Bench is
a client: threads, the queue and the agent live in the agent process and `~/.bench`, so quitting
and reopening it costs the person a few seconds and an unfinished run comes back with the window.
Never kill it or quit it outside `ship.sh`. Rolling back means rebuilding an earlier commit —
nothing is parked beside `/Applications/Bench.app`. Signed with the Apple Development
certificate, so macOS grants are given once per app. Uncommitted changes stop the build
(`BENCH_DIRTY=1` for a throwaway one).
6. **Report honestly** what was verified and what was only built (anything that needs a click).

