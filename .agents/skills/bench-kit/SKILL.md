---
name: bench-kit
description: "Use for any UI or insert change in Bench: the sidebar, thread, settings, Explorer, bench_ui; how to look at a change."
version: 6.0.0
author: The user and their agents (from building Bench, ~/Projects/bench)
license: MIT
platforms: [macos]
metadata:
  tags: [Rust, GPUI, gpui-kit, Desktop, Bench, Pi, Agent-Client, Zed, macOS]
  category: desktop-dev
---

# Bench Kit

How to change what Bench shows. The words are in `docs/concepts.md`, how to work in `AGENTS.md`.

## Where to look, in this order

1. Where the code is: `docs/map.md`.
2. What an element looks like: `crates/storybook/reference/`. The call for each:
   `references/ui-catalog.md`.
3. Whether it exists: `crates/ui`, and the pages below (`ui-overlays.md` for menus). Missing: add
   it to `crates/ui`.
4. How the framework does it: one grep for the name, no reading around. gpui-kit is a facade over
   `gpui-component-0.8.0` (`~/.cargo/registry/src/index.crates.io-*/gpui-component-0.8.0/src`);
   GPUI is `vendor/gpui-base` (patched), its `text_xs`… in `gpui-pre-0.3.9/src/styled.rs`.
5. Why it is built this way: the module's `SPEC.md`, `docs/architecture.md`.

A `Button` that ignores a font size or won't wrap: `ui-gotchas.md` ("A `Button` is not a row of
text").

## The two cores (read before touching them)

- **The thread** — Pi's journal → `crates/thread` rows → `src/thread/` on `gpui::list`.
  Nothing moves it but the person; rows are remeasured in place, never spliced; the room past
  the end; no flash when an entry replaces what was streamed. `references/build-thread.md`.
- **Projects and agents** — `ProjectKey` and `Location` (`src/app/model.rs`), `src/app/places.rs`
  the only place that says where a project lives, `src/app/agents.rs` the links to agents;
  the agent works on a project's machine through Pi's `ExecutionEnv` (`agent/src/machine/ssh.ts`,
  `machine/confined.ts`). `references/build-projects.md`.

## The shape of every change

- **An element or a size** → `crates/ui` (and the storybook). A screen writes no `px(…)` and no
  bare Tailwind spacing (`gap_2`, `px_3`): `.gap(Gap::M)`, `.px(Gap::L)`, `bench_ui::row`/`stack`,
  a named constant in `metrics.rs`.
- **Something clickable** → a `bench_ui` element, or wrapped in `bench_ui::own_press`: what handles
  a click stops its propagation, or the thread takes the press and gives the composer the focus
  (`ThreadView::press`).
- **What the thread shows, its order** → `crates/thread` (rules as tests), drawn by `src/thread/`.
- **Where a project lives, a new kind of place** → `src/app/places.rs` + the agent's project row.
- **A new insert** → a folder in `src/inserts/` (`references/build-insert.md`).
- **The window's places** → `src/workspace/dock.rs` (`Dock`, `Panel`) and `mod.rs` (`Center`), as Zed's workspace;
  **the Explorer** (a project's files and the agent's texts as an IDE shows them) →
  `src/explorer/`, `docs/explorer.md`.
- **A project's Settings or parts** → a file in `src/env/` implementing `Section` and `SettingsForm` (a line in `FORMS`) or `ExplorerParts` (a line in `PARTS`)
  (`src/env/mod.rs`); where it lives: `place.rs`; its data from `/api/projects/<id>/env`.
- **The sidebar** → `src/sidebar/mod.rs` draws; what is done to a thread or project (menu, a
  thread's pin, color, rename, ⌫⌫, ↑↓) is `src/sidebar/items.rs`, marks in `src/sidebar/marks.rs`.
- **The gear** → `src/settings/mod.rs`: tabs Agents (each agent's providers `providers.rs`, voice
  `voice.rs`, evaluator servers `evaluator.rs`, their shared form `server_form.rs`; add/remove a
  server's agent `agent_form.rs`), Presets (`presets.rs`; defaults in
  `agent/src/builtin/presets.ts`), Appearance (`appearance.rs`). Its `SPEC.md` says why.
- **A new area of the agent** → a module in `agent/src/modules/`, a line in `server.ts`.
- **A tool of the agent** → `agent/src/builtin/<tool>.ts`, listed in `extensions/assistant.ts`;
  through `api.env` it works on the project's machine. Code tools: grep, glob, ls
  (`builtin/search.ts`); diagnostics after each edit from language servers
  (`builtin/diagnostics.ts`, `lsp/lsp.ts`).
- **What a turn's hooks keep, the evaluator** → `agent/src/turn/`, `agent/src/judge/` (each has a
  `SPEC.md`).
- **A type Bench and the agent share** → `crates/protocol`; `cargo test -p protocol` writes
  `agent/src/protocol.gen.ts`.

## Working on Bench from inside Bench

The loop is in `AGENTS.md`. What it needs beyond that:

- The demo is a fake agent. Every hook (`BENCH_DEMO=env`, `BENCH_PANEL=providers`,
  `BENCH_DEMO_DRAG=…`) is in one table in `references/build-verification.md`. Every demo state
  against a reference, for a change to many screens: `scripts/pixels/`.
- The picture is taken by macOS's `screencapture`. No picture while the copy started ("no window
  for pid", nothing in its log): ask the person to allow Bench in Privacy & Security → Screen &
  System Audio Recording, once.
- From outside Bench, `scripts/ship.sh` quits Bench the honest way, rebuilds it and opens it.
- A Bench opened while the bundle is being written starts on half of it: build again.

## Pages

| Page | What |
|---|---|
| `../bench-structure/SKILL.md` | How to grow Bench without patches (in Russian): the forecast and the three cores first, then a file tree readable without docs, then Rust and Zed words |
| `references/build-thread.md` | The thread core: pipeline, what the view promises, how to see it |
| `references/build-projects.md` | Projects, agents, machines: the model, the agent side, growing it |
| `references/ui-catalog.md` | Every element: the call and its file |
| `references/build-insert.md` | A new kind of insert, end to end, with the three kinds (terminal, browser, tools) as examples |
| `references/build-architecture.md` | The crates, the app's folders, the agent's modules |
| `references/build-pi-backend.md` | The agent on Pi Durable: tools, prompt sections, how each insert uses Pi |
| `references/build-change-loop.md`, `build-verification.md` | One change with before/after pictures and measurements; the demo, the one table of its hooks, the pixel check |
| `references/build-native-data.md` | Where a tool's data lives (`~/.bench`) |
| `references/build-crates.md`, `build-media.md`, `build-packaging.md` | Libraries, media, the .app |
| `references/build-hard-won-rules.md`, `build-workflow.md`, `build-custom-render.md` | Rules each paid for by a round of feedback; how a piece of work goes; a custom element |
| `references/design-*.md` | The look's rules and the person's preferences |
| `references/ui-areas.md`, `ui-content.md`, `ui-overlays.md`, `ui-gotchas.md` | Where things go in the window, editors and Markdown, menus and popovers, pitfalls |
| The skill `gpui-kit` | The framework's own reference: only for what none of the above answers |
