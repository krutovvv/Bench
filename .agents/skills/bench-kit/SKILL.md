---
name: bench-kit
description: "Bench Kit: how to change Bench, a native workbench for working with an AI agent (Rust + GPUI, gpui-kit 0.8) on its own agent in agent/ (Pi Durable). The two cores — the thread (Pi's journal as rows on a list that only the person moves) and projects/agents/machines (where each project lives) — where each thing is (docs/map.md), the elements every screen is built from (crates/ui, the storybook), how to change the UI (sections, inserts, the sidebar, the gear), the agent's modules and tools, the change loop with screenshots, shipping. Use for any change to Bench (~/Projects/bench)."
version: 6.0.0
author: The user and their agents (from building Bench, ~/Projects/bench)
license: MIT
platforms: [macos]
metadata:
  tags: [Rust, GPUI, gpui-kit, Desktop, Bench, Pi, Agent-Client, Zed, macOS]
  category: desktop-dev
---

# Bench Kit

Bench is a thread-centric app: a thread is a document of messages and the person's **inserts**
(live tools put in with `/`); a **project** belongs to an **agent** (this Mac's or a server's)
and says where its work is; its **environment** is edited in its own thread. The repository's
`AGENTS.md` has the concepts and the rules; this kit has the how.

## Find before you read

1. **Where is X?** `docs/map.md` answers it in one table. Open that file, not its neighbours.
2. **What does an element look like?** `cargo run -p storybook` or `crates/storybook/reference/`;
   the call for each: `references/ui-catalog.md`.
3. **Why is it built this way?** `docs/architecture.md`, `docs/thread-rewrite.md`,
   `docs/remote-projects.md`.

Never search `~/.cargo/registry` for how gpui-kit draws something: the answer you need is a
constant or an element in `crates/ui`, or a page here (`ui-overlays.md` for menus). If it is
missing there, add it there. When the framework's own source must be read: gpui-kit is a facade
over `gpui-component-0.8.0` (`~/.cargo/registry/src/index.crates.io-*/gpui-component-0.8.0/src`);
gpui itself is `vendor/gpui-base` (patched). Go there directly, with one grep.

## The two cores (read before touching them)

- **The thread** — Pi's journal → `crates/thread` rows → `src/ui/thread/` on `gpui::list`.
  Nothing moves it but the person; rows are remeasured in place, never spliced; the room past
  the end; no flash when an entry replaces what was streamed. `references/build-thread.md`.
- **Projects and agents** — `ProjectKey` and `Location` (`src/model.rs`), `src/app/places.rs`
  the only place that says where a project lives, `src/app/agents.rs` the links to agents;
  the agent works on a project's machine through Pi's `ExecutionEnv` (`agent/src/ssh-env.ts`,
  `confined-env.ts`). `references/build-projects.md`.

## The shape of every change

- **An element or a size** → `crates/ui` (and the storybook). Never `px(…)` in a screen.
- **What the thread shows, its order** → `crates/thread` (rules as tests), drawn by `src/ui/thread/`.
- **Where a project lives, a new kind of place** → `src/app/places.rs` + the agent's project row.
- **A new insert** → a folder in `src/inserts/` (`references/build-insert.md`).
- **A project's environment** → a file in `src/env/` implementing `Section`, a line in `SECTIONS`
  (`src/env/mod.rs`); where it lives: `place.rs`; its data from `/api/projects/<id>/env`.
- **The sidebar** → `src/ui/sidebar.rs` draws; what is done to a thread or project (menu, pin,
  color, rename, ⌫⌫, ↑↓) is `src/app/sidebar_items.rs`, marks in `src/app/marks.rs`.
- **The gear** → `src/ui/providers.rs`: tabs Agents (each agent's models and voice, add/remove a
  server's agent: `src/app/agent_form.rs`), Presets (`src/app/presets.rs`; defaults in
  `agent/src/builtin/presets.ts`), Appearance (`src/ui/panels.rs`).
- **A new area of the agent** → a module in `agent/src/modules/`, a line in `server.ts`.
- **A tool of the agent** → `agent/src/builtin/<tool>.ts`, listed in `extensions/assistant.ts`;
  through `api.env` it works on the project's machine. Code tools: grep, glob, ls
  (`builtin/search.ts`); diagnostics after each edit from language servers (`lsp.ts`).
- **A type Bench and the agent share** → `crates/protocol`; `cargo test -p protocol` writes
  `agent/src/protocol.gen.ts`.

## Working on Bench from inside Bench

- **Commit every change; build only when asked.** As Bench's own agent: `scripts/bundle-macos.sh`
  (builds `/Applications/Bench.app` in place, quits nothing), then the person quits and reopens
  Bench. From outside Bench: `scripts/ship.sh` (quits Bench the honest way, rebuilds, opens it).
  Never `pkill` Bench. A Bench opened while the bundle is being written starts on half of it:
  build again.
- **See every UI change before you report it — on your own copy, not the person's Bench.**
  `cargo build`, then `scripts/shot.sh target/debug/bench /tmp/after.png 10 BENCH_DEMO=landing`:
  it starts a separate copy of the debug binary (no app, no Dock icon, no permissions asked) on
  made-up data, takes the picture, quits the copy. Read the picture; change, build, shoot again
  until it is right. No `.app` build and no restart of Bench is needed for this. The demo's
  threads and every hook (`BENCH_PANEL=providers`, `BENCH_DEMO=env`…): `references/build-verification.md`.
- **Checks**: `cargo build` (zero warnings), `cargo test --workspace`,
  `cd agent && node --test test/*.test.ts`.
- **Back up before a migration or a deletion** of the person's data (`~/.bench/backups/`).
- **Small steps**: one change, build, look, commit. Past 40 tool calls without a running result:
  stop and say what is hard.

## Pages

| Page | What |
|---|---|
| `references/build-thread.md` | The thread core: pipeline, what the view promises, how to see it |
| `references/build-projects.md` | Projects, agents, machines: the model, the agent side, growing it |
| `references/ui-catalog.md` | Every element: the call and its file |
| `references/build-insert.md` | A new kind of insert, end to end, with the six kinds as examples |
| `references/build-architecture.md` | The crates, the app's folders, the agent's modules |
| `references/build-pi-backend.md` | The agent on Pi Durable: tools, prompt sections, how each insert uses Pi |
| `references/build-change-loop.md`, `build-verification.md` | One change with before/after pictures and measurements; the demo and its hooks |
| `references/build-native-data.md` | Where a tool's data lives (`~/.bench`) |
| `references/build-crates.md`, `build-media.md`, `build-packaging.md` | Libraries, media, the .app |
| `references/design-*.md` | The look's rules and the person's preferences |
| `references/ui-areas.md`, `ui-content.md`, `ui-overlays.md`, `ui-gotchas.md` | Where things go in the window, editors and Markdown, menus and popovers, pitfalls |
| `references/gpui-*.md` | The framework's own reference — only for what none of the above answers |
