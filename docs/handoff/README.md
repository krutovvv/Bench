# Handoff: the whole-project audit and its fixes (paused 2026-10-30)

For the agent that continues. Read in this order: AGENTS.md, `docs/handoff/principles.md` (the
owner's rules, which decide every judgment call below), this file, then the skill for the area you
touch (`.agents/skills/bench-kit/SKILL.md` for UI and the build loop, `.agents/skills/bench-structure/SKILL.md`
for structure, `.agents/skills/pi-durable-dev-rules/SKILL.md` for the agent). The owner writes in
Russian; answer in Russian.

## Where things stand

- `main` (local, nothing pushed — the repository is public; push only when the owner asks) holds
  everything done so far, merged and green: `cargo build` with zero warnings, `cargo test
  --workspace`, `cd agent && node --test test/*.test.ts` (170 pass), `node scripts/specs-check.mjs`.
- `/Applications/Bench.app` was last bundled at 196651e: wave B and wave C's commits are **not**
  in the running app yet. Bundle with `scripts/bundle-macos.sh` once the pictures are checked (see
  below), then ask the owner to restart Bench ("собрано — перезапусти Bench"). Never quit or kill
  Bench yourself.
- Branch `wip/insert-body-inset` (one commit, 62157a5) parks wave C item 2 half done and
  unverified — continue it or redo it.
- Branch `agent-refactor-wave-b` is merged; the worktree `.claude/worktrees/cranky-shannon-8849b6`
  can be removed (`git worktree remove`).
- Disk is tight (~37 GB free, `target/` ~17 GB): never `cargo clean` everything without reason,
  never a release build, never a second target dir (so no Rust work in a separate worktree).

## What was done in this session (2026-10-29/30), in order

1. f9a8c1c — code mode is every thread's; the per-project setting is gone.
2. 2c3d1fc — the gear opened from the Code view closes the Explorer first (`leave_code_for_gear`
   in `src/workspace/mod.rs`): closing it leaves the chat whole.
3. f789d06 — the evaluator is pi-ai's classifier `bench/evaluator` (`agent/src/judge/models.ts`):
   the gear's servers list (`src/settings/evaluator.rs`, REST `GET /api/judge`,
   `PUT/DELETE /api/judge/server`, `POST /api/judge/main|fallback` in `agent/src/modules/settings.ts`),
   main + fallback; code mode gets `models.*`; a script's `models.classify()` on `bench/evaluator`
   is refused with the reason where the project's evaluator is off; new projects have it on
   (`agent/src/builtin/projects.ts`); MCP follows Pi's `exposure` with `codemode` default
   (`agent/src/mcp/mcp.ts`); each server is a code mode namespace; MCP results reach scripts whole
   (`structuredOutput`). The old one-server `judge` setting reads as one server.
4. bed7607 — a script's evaluator questions count in the thread's "Evaluator calls" meter.
5. 196651e — fixes after the audit: the `/browser` insert's server is `direct` by default; the
   evaluator's MCP hints use Pi's sanitized prefix and say how a tool is reached; a server whose
   `tools/list` failed is retried; `tool_search` is offered only where some tools are `deferred`.
6. Wave B (13 commits, merged at 2b53714): the agent's refactors — see "Wave B" below.
7. Wave C, 6 of 14 items (25c4568 … 22e883f) — see "Wave C" below.

Lessons worth knowing: in code mode scripts the catalog is async — `await
models.getModelOfType("classifier", "bench", "evaluator")`; pi-ai's System One parser requires an
answer for every question asked and a `bool` question needs `criteria: {true, false}`; the owner's
server (old setting migrated, `192.168.1.100:10292`, model `h2oai/h2o-lightning-4b`) answers bool,
choice and score through pi-ai in ~160 ms. A worktree made by the Agent tool started from an old
commit once — check `git log -1` in any worktree before work. `agent/test/process.test.ts` fails
when a leftover `sleep 3032` from an earlier run is alive: kill it.

## The task: fix everything the audit found

Three read-only reviews (agent, Rust app, documents) produced the list below. The owner decided:
**do all of it, waves A–D**, keeping pixels identical; if a mechanical cleanup breaks a detail,
they will fix it later. Order: finish C, then D, then A (documents last, so they describe the
final code). One change, one commit (message: a sentence saying what is now true, a short body,
the attribution line from the session), then bundle.

### Pixel checks

`docs/handoff/pixels/`: `states.txt` (28 demo states, one per line: name and env), `shoot.sh <dir>`
(shoots every state from a copy of `target/debug/bench` into `$PIXELS_DIR/<dir>`, default
`/tmp/bench-pixels`), `cmp.py <dir>` (compares `<dir>` against `before/` and `before2/`, masking
the traffic lights, the sidebar fade and scrollbars; needs Pillow). The reference shots from before
wave C were in the session scratchpad and may be gone: take new references on the commit before
your change (`git stash`-free: check out the earlier commit, build, shoot `before` and `before2`,
come back), then shoot `after` and compare. **Wave C's six commits were not compared in full** —
compare 196651e against `main` once before bundling.

### Wave B — agent (DONE, merged)

dead code removed (Live.abort/generation, signedIn, findSkill, call.args, the TS6133);
`runtime/turns.ts` → `runtime/changes.ts`; the hooks' state of a turn moved to `agent/src/turn/`
(turn, window, steps, frozen, compaction) with one per-conversation store freed with the thread
(`turn/kept.ts`) and a SPEC; Pi's summary prefix one constant; `judge/boundary.ts` reads Pi's
policy; points pass state from `ask` to `apply` as context (no maps, a race fixed);
`evaluatorFor(agent)` + `judgeActive(conversation)` the only predicates; the plan read once and
`judge/SPEC.md` says who owns stop/done (the course check no longer asked after the step limit);
the evaluator's state sized from its window; a result's notes are appended after the output is
fitted; `shOut` → `agent/src/machine/sh.ts`; `extensions/assistant.ts` only wires hooks
(`builtin/approvals.ts`, `builtin/diagnostics.ts`, `extensions/spec-note.ts`); MCP namespaces leave
with their servers; `prompt.ts` names scripted servers via `scriptedServers(agent)` in `mcp.ts`.

Left from B for wave A: AGENTS.md still names `builtin/steps.ts`, `builtin/frozen.ts`,
`builtin/window.ts`; `docs/map.md` names `runtime/turns.ts`; Rust comments name
`runtime/turns.ts`/`rt.turns` (`src/explorer/pane/doc.rs`, `src/explorer/code/turn_diff.rs`,
`src/thread/rows/changed_files.rs`, `src/client/rest/files.rs`); `build-pi-backend.md` names old
builtin paths.

### Wave C — Rust: one implementation per pattern, pixels unchanged

Done: 12 stale comments (25c4568); 11 names — `rows/answer.rs`→`clarify.rs`, `extras.rs`→`pages.rs`,
the line-stats file, `env` `Item`→`PartRow`, `Request` renamed, `env/mcp/` folder (e956aa1);
10 tool labels in `crates/thread` with one URL host parser (fea84d1); 13 `BenchApp` one field per
area, `restart_armed` folded into `armed` (66edf69); 4 the `~/.ssh/config` servers are the places'
list, inserts' state named after the kinds (160f9a2); 3 each insert kind says where it is offered,
its "/" key and its welcome chip (22e883f).

To do:
1. The gear's provider card (`src/settings/mod.rs` render, handlers `src/settings/providers.rs`)
   and evaluator server card (`src/settings/evaluator.rs`) are one card written twice: one
   `bench_ui` card (title + role word, `settings_field` rows, a check line, actions) and an
   "Add a …" link; one shared form shape; provider rendering moves into `providers.rs`.
2. Insert bodies undo the block padding with negative margins (`src/inserts/mod.rs` px_3/py_2;
   `inserts/tools/mod.rs`, `inserts/terminal/mod.rs` `mx(px(-12.))`; `thread/rows/queued.rs`):
   the kind says whether its body is inset. Half done on `wip/insert-body-inset`.
5. `segmented`, `segmented_owned`, `segmented_tall_owned` live in `src/inserts/tools/view.rs` but
   the gear uses them: move into `crates/ui` beside `bench_ui::segmented`; drop the wrapper
   closures in `src/env/run.rs`, `src/settings/appearance.rs`.
6. "Show all / Show less" footer copied ~5× (`src/media/view.rs` ×2, `src/thread/rows/steps.rs` ×2,
   `src/thread/rows/changed_files.rs`): one `bench_ui::fold_more(id, open, label, size, on_toggle)`.
7. The code widget: `src/media/text.rs` `render_code` with `FOLD_PREVIEW_H` (a pixel value outside
   crates/ui); `bench_ui` owns one `code_frame` (frame, head, body, fold) for answer code blocks and
   step cards; also the hand-made code frames (`thread/rows/asks.rs` detail box,
   `env/schedule.rs` editor frame, the terminal) where pixels stay the same.
8. The bordered chip hand-built ~7× (`env/lsp.rs`, `env/mcp/` preset chips, `inserts/tools/view.rs`
   regex flag and `small_button`, the pages chip, the terminal's Background badge): `bench_ui::chip`.
9. Two diff panes (`src/explorer/code/git.rs` `diff_pane`, `src/explorer/code/turn_diff.rs`): one.
14. The agent popover (`src/sidebar/agent.rs` ~140-290) repeats a section 4×:
    `bench_ui::popover_section(title, body)`.
Add every new `bench_ui` component to the storybook page (`crates/storybook`).

### Wave D — Rust: causes, not patches (the owner wants all of it)

1. **The demo as a fake backend, as Zed's `FakeFs`.** 38 `if self.demo`/`app.demo` branches in
   product code (e.g. `inserts/terminal/mod.rs`, `inserts/browser/mod.rs`, `explorer/code/git.rs`,
   `lsp.rs`, `problems.rs`, `thread/open.rs`, `thread/rows/changed_files.rs`,
   `settings/providers.rs`, `sidebar/agent.rs`; grep `\.demo\b`) and `std::env::var("BENCH_DEMO_*")`
   / `BENCH_TOOLS_SHOWN` read in render paths every frame (`workspace/layout.rs`,
   `thread/rows/work.rs`, `thread/composer/input.rs`, `sidebar/agent.rs`). Make `src/demo/` a
   client backend that answers RPC and REST from fixtures behind the same interface as
   `src/client/` (`rpc/`, `rest/`), so product code never branches; read the env hooks once at
   start into a `DemoHooks` owned by `src/demo/`. Every `BENCH_DEMO…` state must still render as
   before (the pixel set above).
2. **`click_claimed` → GPUI propagation.** An app-wide flag (`src/app.rs`, read in
   `src/thread/mod.rs` ~166) set from ~47 places so a click inside a thread does not steal focus to
   the composer. Interactive children stop propagation on mouse down (`cx.stop_propagation()`,
   built into `bench_ui` buttons, chips, fold heads); the thread focuses the composer only on an
   unhandled mouse up when focus did not change. Remove the flag and every `claim_click`.
3. **Side effects in `Render`** (`src/workspace/layout.rs` ~72-95: loading prompts on a project
   change, demo hooks, the browser stream, nav history `note_place`): move to observers /
   subscriptions as Zed does (`cx.observe`, subscribe to the active item).
4. **Pixel values and spacing outside `crates/ui`**: 335 Tailwind spacing steps in `src/`
   (`px_3`, `gap_2`, …; top files `thread/rows/asks.rs`, `media/view.rs`, `settings/new_project.rs`,
   `sidebar/agent.rs`, `thread/rows/steps.rs`, `inserts/tools/view.rs`), literal `px(…)`
   (`sidebar/menu.rs`, `thread/list.rs`, `asks.rs`, `composer/input.rs`, `inserts/tools/mod.rs`),
   f32 size constants (`media/view.rs`, `inserts/browser/view.rs`, `inserts/terminal/mod.rs`,
   `inserts/tools/mod.rs`, `composer/input.rs`). `Gap`, `row`, `stack`, `spread`
   (`crates/ui/src/layout.rs`) exist and are unused in `src/`. Move sizes into `crates/ui`
   (`metrics.rs`), convert to `Gap` and the layout helpers mechanically, file by file, with pixel
   checks. Many may be garbage: delete what does nothing.
5. **Polling instead of events**: `src/app/subscriptions.rs` ~76-90 checks every 500 ms whether
   playback ended: the player should notify.

### Wave A — documents, specs, skills (last)

Every item verified against the code by the review (fix after C and D, recheck paths then):
- **bench-kit skill:** `SKILL.md`, `build-change-loop.md`, AGENTS.md send readers to
  `build-verification.md` for demo hooks; it has none. Move the hooks list (AGENTS.md's long
  `src/demo/` cell) into one table in `build-verification.md`, including undocumented ones:
  `BENCH_DEMO_ARM`, `_DRAFT`, `_FILES`, `_PAUSED`, `_POP=agent`, `_REVIEW`, `_SEARCH`,
  `_SIDE=threads`, `_WORK_DOWN`, `BENCH_GEAR_TAB`, `BENCH_ENV`, `BENCH_PREFILL`,
  `BENCH_CLARIFY_MULTI`, `BENCH_TOOLS_SHOWN` (after wave D, document `DemoHooks` instead).
  Temporary-hook examples use removed task code (`open_task_list`, `open_subject_key`,
  `self.sessions`): replace. `BENCH_DEMO=tasks` and the grep for `"Search sessions` are gone.
  Contradictions on commit/bundle timing (`build-verification.md` ~99 "only if asked",
  `build-change-loop.md` ~204, ~190): AGENTS.md's loop is right. Script paths: write
  `.agents/skills/bench-kit/scripts/…` everywhere. Insert kinds are three (terminal, browser,
  tools), not six or four. Add `src/settings/evaluator.rs` to SKILL.md. `build-agent-client.md` is
  Hermes-era and linked from nowhere: delete or rewrite from `crates/protocol/SPEC.md`;
  `build-hard-won-rules.md` is linked from nowhere. `build-pi-backend.md`: `profile`→`agent=`,
  remove tasks (`bench.sqlite` tasks, `/api/tasks`), extensions include `codemode`, path fixes,
  `PROBE_PROFILE` not read. `build-architecture.md`: real `app/` file list, real agent paths,
  `judge/`, `mcp/`, `turn/`. Stale paths: ui-gotchas.md (statusbar), ui-catalog.md (theme),
  ui-content.md (diff.rs, `render_session_files`, `open_in_zed`, `env/ide.rs`, "Open in Zed"
  contradicts "no external editor"), ui-overlays.md (composer, processes), build-media.md,
  build-crates.md.
- **Insert RPC** is `thread.insert` / `thread.insert.patch` (`crates/protocol/src/rpc.rs`), told
  as `ThreadEvent::Inserts` — not `session.insert*`: AGENTS.md, `build-insert.md`,
  `build-pi-backend.md`, `src/inserts/SPEC.md`.
- **docs/map.md:** evaluator row → `src/settings/evaluator.rs`, `agent/src/judge/models.ts`,
  `/api/judge`; gear row + `evaluator.rs`; the opened work → `src/thread/rows/steps.rs` and
  `crates/thread/src/steps.rs`; new rows for MCP exposure and code mode; the tree (app/, modules/,
  runtime/, judge/, mcp/, machine/, lsp/, turn/, docs/).
- **AGENTS.md:** thread rows list (steps, clarify, welcome; tool labels now in crates/thread),
  settings row (+ evaluator.rs, state.rs), builtin paths that moved to `agent/src/turn/`, the
  Evaluator row shortened to the Pi mapping plus a link to `agent/src/judge/SPEC.md`.
- **Specs:** "subagents" listed as present in root `SPEC.md` and `agent/SPEC.md` (removed);
  `agent/SPEC.md` prompt sections are frozen until compaction (not "resent when changed"), judge is
  `bench/evaluator`, extensions always include code mode (+ tool search with deferred tools), add a
  decision bullet or `agent/src/mcp/SPEC.md` owning the `exposure` default; root SPEC capabilities
  gain the evaluator, code mode, the step limit; `crates/thread/SPEC.md` ~44 the reasoning is shown
  whole now (since b5dce58) — rewrite, and `steps::beat` is used only by tests; `src/workspace/SPEC.md`
  gets the "gear belongs to the chat" decision; a SPEC for `src/settings/`.
- **docs/explorer.md:** the center is `Chat | Doc | Settings | NewProject`, the right panel
  `Panel::Thread`; paths from before the pane/tree/code/search split.
- **README.md:** the fallback model is the gear's, not per project.
- **docs/architecture.md** (Russian): §1 and §3 describe the old state as current (100-field
  `BenchApp`, Tasks ⌘4, a `tasks` module): label them as history or trim to §6.
- **bench-structure:** `src/explorer/item.rs` → `src/explorer/pane/item.rs`.
- **pi-durable-dev-rules:** it describes Pi 1.2.0; installed is 1.4.0 (run its `scripts/refresh.sh`
  or say the snapshot's age); `SKILL.md` points to a non-existent `pi-dev-rules` for MCP; bundle
  Pi's `docs/mcp.md` (exposure) — https://pi.dev/docs/latest/mcp.
- **One place per fact** (they already drifted): source layout → `docs/map.md` (AGENTS.md keeps a
  short summary and links); the build loop → bench-kit `SKILL.md` once; the evaluator →
  `agent/src/judge/SPEC.md`; demo hooks → one table; insert RPC → `src/inserts/SPEC.md` + the Pi
  table. Remove this `docs/handoff/` folder when the task is finished (or keep `principles.md`
  folded into AGENTS.md/bench-structure).
