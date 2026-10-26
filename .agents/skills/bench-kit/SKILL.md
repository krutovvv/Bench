---
name: bench-kit
description: "Bench Kit: how to change Bench, a native workbench for working with an AI agent (Rust + GPUI, gpui-kit 0.8) on its own agent in agent/ (Pi Durable). Where each thing is (docs/map.md), the elements every screen is built from (crates/ui, the storybook), the recipe for a new insert (a folder in src/inserts/) and a new environment section (a file in src/env/), the agent's modules, the change loop with screenshots, shipping. Use for any change to Bench (~/Projects/bench)."
version: 5.0.0
author: The user and their agents (from building Bench, ~/Projects/bench)
license: MIT
platforms: [macos]
metadata:
  tags: [Rust, GPUI, gpui-kit, Desktop, Bench, Pi, Agent-Client, Zed, macOS]
  category: desktop-dev
---

# Bench Kit

Bench is a thread-centric app: a thread is a document of messages and the person's **inserts**
(live tools put in with `/`), and a project's **environment** is edited in its own thread. The
repository's `AGENTS.md` has the concepts and the rules; this kit has the how.

## Find before you read

1. **Where is X?** `docs/map.md` answers it in one table: every folder, what it holds, the file
   for each common change. Open that file, not its neighbours.
2. **What does an element look like?** `cargo run -p storybook` or the pictures in
   `crates/storybook/reference/`. The call for each: `references/ui-catalog.md`.
3. **Why is it built this way?** `docs/architecture.md`.

Never search `~/.cargo/registry` for how gpui-kit draws something: the answer you need is a
constant or an element in `crates/ui`. If it is missing there, add it there.

## The shape of every change

- **An element or a size** → `crates/ui` (and the storybook). Never `px(…)` in a screen.
- **What the thread shows, its order** → `crates/thread` (rules as tests), drawn by `src/ui/thread/`.
- **A new insert** → a folder in `src/inserts/` (`references/build-insert.md`).
- **A new environment section** → a file in `src/env/` implementing `Section`.
- **A new area of the agent** → a module in `agent/src/modules/`, a line in `server.ts`.
- **A type Bench and the agent share** → `crates/protocol`; `cargo test -p protocol` writes
  `agent/src/protocol.gen.ts`.

## Working on Bench from inside Bench

- **Commit every change; build only when asked.** `scripts/ship.sh` quits Bench the honest way,
  rebuilds `/Applications/Bench.app` and opens it. Never `pkill` Bench.
- **Screenshots only of your own copy**: `scripts/shot.sh target/debug/bench out.png 8 BENCH_DEMO=top`
  (`BENCH_DEMO=1|top|tasks|server|tool|env|env-open`, `src/demo.rs`).
- **Checks**: `cargo build` (zero warnings), `cargo test --workspace`,
  `cd agent && node --test test/*.test.ts`.
- **Small steps**: one change, build, look, commit. Past 40 tool calls without a running result:
  stop and say what is hard.

## Pages

| Page | What |
|---|---|
| `references/ui-catalog.md` | Every element: the call and its file |
| `references/build-insert.md` | A new kind of insert, end to end, with the six kinds as examples |
| `references/build-architecture.md` | The crates, the app's folders, the agent's modules |
| `references/build-pi-backend.md` | The agent on Pi Durable: tools, prompt sections, how each insert uses Pi |
| `references/build-change-loop.md`, `build-verification.md` | One change with before/after pictures and measurements |
| `references/build-native-data.md` | Where a tool's data lives (`~/.bench`) |
| `references/build-crates.md`, `build-media.md`, `build-packaging.md` | Libraries, media, the .app |
| `references/design-*.md` | The look's rules and the person's preferences |
| `references/ui-areas.md`, `ui-content.md`, `ui-overlays.md`, `ui-gotchas.md` | Where things go in the window, editors and Markdown, menus and popovers, pitfalls |
| `references/gpui-*.md` | The framework's own reference — only for what none of the above answers |
