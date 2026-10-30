---
id: app
type: module-design
status: draft
title: src/ — the Bench app (the window and its areas)
parent: bench
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
- **`demo/`**: a fake agent for screenshots and the web demo (`BENCH_DEMO=…`), as Zed's `FakeFs`:
  installed at start behind `client/fake.rs`, it answers from fixtures, and what its hooks ask of the
  window is done from outside (`script.rs`), read once at start (`hooks.rs`). Product code never asks
  whether it is the demo. Its state goes to a temporary folder of its own.

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
  thread's events to every view showing it); there are no status timers, and what ends says so (a
  clip: `Playback::ended`).
- **`Render` draws and does nothing else.** What follows a change (the prompts `@` offers, the
  browser's stream, the place shown) follows it from an observer (`BenchApp::observe_changes`,
  `follow_shown` in `app/subscriptions.rs`), as Zed subscribes to the active item.
- **What handles a click stops its propagation**: GPUI's own mechanism, built into `bench_ui`'s
  clickable elements (`own_press`); no app-wide flag claims a click.
- **One mechanism per job**: one `armed` for every "press again to confirm", one way to open a
  file (a tab), one save path per entity.

## Dependency edges between areas

`workspace` decides which panel or item is shown; each area draws itself. `thread` (with its
composer) uses `inserts` and `media`; `explorer` uses `env` (its parts) and opens the thread
beside through `workspace`; `env` and `settings` reach the agent through `client`; every area
reads "where" from `app/places.rs` and draws with `ui`. Child specs: `workspace`, `thread-view`,
`inserts`, `explorer`, `env`, `client`, `settings`.

## Known limitations

`InsertKind` and `Section` still take `&mut BenchApp`, so inserts and sections are not yet
entities or crates of their own; cards are written as code, not as field schemas. Where this
is going is `architecture` (§6 "Where we are").
