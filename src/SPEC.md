---
id: app
type: module-design
status: draft
title: src/ — the Bench app (the window and its areas)
parent: architecture
depends-on: [ui, protocol, thread-core, devtools]
references: [projects-and-agents]
---

# The Bench app

The `bench` crate: one macOS window (more with ⌘⇧N) laid out as Zed's workspace, talking to one
or more agents. Everything that is drawn, and every service the drawing needs, lives here;
what can be pure and tested without GPUI is pushed down into `crates/` (`thread-core`,
`protocol`, `devtools`, `ui`).

## How it is cut

Only `main.rs` and `app.rs` are loose in `src/`; every other file is in a folder named for where
it is in the window or what it does, so a request ("fix the title bar") leads to a folder by
its name alone (`.agents/skills/bench-structure/SKILL.md` §2 owns that rule and the file-size
rule).

- **By place in the window**: `workspace/` (docks and center), `titlebar/`, `sidebar/`,
  `thread/`, `settings/` (the gear and the new project's page). The chat has no status bar (the thread's
  project, repo and meters are its composer's foot, the agent the left dock's foot); the Code
  view's is `explorer/statusbar.rs`.
- **By feature, spanning places**: `explorer/`, `inserts/`, `env/`.
- **Services that draw nothing**: `client/` (the agent), `media/`, `system/` (ssh, the
  `~/.ssh/config` servers, notifications).
- **`app/`** holds only what belongs to the whole app: the plain data every area uses
  (`model.rs`: `ProjectKey`, `Location`, `Target`), the agents' events applied to the window
  (`events.rs`), keys and the macOS menu, the second-press confirmations (`armed.rs`), and the
  two answers below.
- **`demo/`**: a made-up session for screenshots (`BENCH_DEMO=…`). Product code does not guard
  its own writes against it: the demo points the state folder at a temporary one of its own.

## Invariants

- **State lives in its area.** `BenchApp` holds one field per area (`app.sidebar`,
  `app.threads`, `app.inserts`, `app.media`, `app.gear`…); a new piece of state goes into its
  area's struct (`<area>/state.rs` or `store.rs`), never as a new field of `BenchApp`. Per-project
  state is keyed by its project (`envs`, by the project's key), never one slot for "the current
  one".
- **One answer to "where"**: `app/places.rs` says which agent runs a project's threads, which
  machine its files and commands are on, which folder (`Location`); nothing else works it out.
  `app/agents.rs` holds one link per agent, all alike, and sends a request to the agent it
  concerns (`projects-and-agents`).
- **A thread on screen is its own entity** (`thread-view`): what it asks of the app names the
  thread with a `ThreadRef` and never reads a view.
- **Events, not polls.** The window changes when an agent says so (`app/events.rs` routes a
  thread's events to every view showing it); there are no status timers.
- **One mechanism per job**: one `armed` for every "press again to confirm", one way to open a
  file (a tab), one save path per entity.

## Dependency edges between areas

`workspace` decides which panel or item is shown; each area draws itself. `thread` (with its
composer) uses `inserts` and `media`; `explorer` uses `env` (its parts) and opens the thread
beside through `workspace`; `env` and `settings` reach the agent through `client`; every area
reads "where" from `app/places.rs` and draws with `ui`. Child specs: `workspace`, `thread-view`,
`inserts`, `explorer`, `env`, `client`.

## Known limitations

`InsertKind` and `Section` still take `&mut BenchApp`, so inserts and sections are not yet
entities or crates of their own; cards are written as code, not as field schemas. Where this
is going is `architecture` (§6 "Where we are").
