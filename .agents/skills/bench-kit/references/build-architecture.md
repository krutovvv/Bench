# How Bench is put together

The whole picture — what Bench is, where it will grow, and the structure that takes that growth —
is `docs/architecture.md` in the repository. Where each thing is: `docs/map.md`. Agents, machines
and projects on servers: `docs/remote-projects.md` (and `build-projects.md`). In short:

- **Crates** (`crates/`): `ui` (every element and size), `storybook` (all of them on one page),
  `protocol` (what Bench and the agent say about a thread; TypeScript generated for the agent),
  `thread` (the thread as rows, from Pi's journal, with the rules of order as tests),
  `devtools` (the /tools utilities as pure functions). None of them knows `BenchApp`.
- **The app** (`src/`):
  - `app.rs` and `app/` — the window's state and actions, one file per area (`events`,
    `sessions`, `send`, `keys`, `slash`, `voice`, `projects`, `providers`…);
  - `model.rs` — `ProjectKey` (a project's agent and its id there) and `Location` (agent,
    machine, folder);
  - `app/places.rs` — the only place that says where a project lives; it puts the sidebar's
    projects together from their sources;
  - `app/agents.rs` — the links to agents (this Mac's, key `""`, and servers'), `conn_of`,
    `route`; `settings/agent_form.rs` adds and removes a server's agent in the gear;
  - `sidebar/items.rs` — what is done to a thread or project in the sidebar (pin, color,
    rename, ⌫⌫, ↑↓); `sidebar/marks.rs` the pins and colors, kept on this Mac;
  - `settings/presets.rs` — the person's presets, edited in the gear;
  - the window by area: `workspace/` (docks and center, the layout), `titlebar/`, `sidebar/`,
    `thread/` (the open thread, the composer), `settings/` (the gear);
    `media/` an answer's media; `system/` ssh and notifications; `demo/` the made-up session;
  - `inserts/` — one folder per insert kind; `env/` — a project's environment, one file per section;
  - `client/` — the protocol to the agent: `rpc/` (WebSocket JSON-RPC) and `rest/` (REST);
    `connection.rs`, `launch.rs` and `service.rs` — where an agent runs, installing it and
    bringing it up as its machine's service, one script here or over SSH (`ensure(target)`), and
    its tunnel.
- **The agent** (`agent/src/`):
  - `server.ts` collects `modules/*.ts` (each declares its RPC methods and REST routes);
  - `runtime.ts` runs Pi conversations (`envFor`: each one's `ExecutionEnv`); `thread.ts` turns
    Pi's journal into the items Bench shows;
  - `machine/ssh.ts` — a project's machine over SSH as Pi's `ExecutionEnv`; `machine/confined.ts` — a
    project kept in its folder; `lsp.ts` — its language servers (diagnostics after every edit),
    `lsp/presets.ts` the ones it can take with one click;
  - `builtin/` — the agent's own tools and stores; among them `search.ts` (grep, glob, ls),
    `servers.ts` (the list of servers' agents, setting `agents`, `/api/agents`), `presets.ts`
    (the presets' defaults, setting `presets`).

The rules: a crate depends only on crates below it; pixel values live only in `crates/ui`; "where"
is answered only by `places.rs`; a new insert is a folder in `src/inserts/`, a new environment
section a file in `src/env/`, a new area of the agent a module in `agent/src/modules/`.
