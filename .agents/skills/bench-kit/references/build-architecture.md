# How Bench is put together

The whole picture — what Bench is, where it will grow, and the structure that takes that growth —
is `docs/architecture.md` in the repository. Where each thing is: `docs/map.md` (the one source for
the source layout). Agents, machines and projects on servers: `docs/remote-projects.md` (and
`build-projects.md`). Each module's why: its `SPEC.md`. In short:

- **Crates** (`crates/`): `ui` (every element and size), `storybook` (all of them on one page),
  `protocol` (the wire: one type per method; TypeScript generated for the agent),
  `thread` (the thread as rows, from Pi's journal, with the rules of order as tests),
  `devtools` (the /tools utilities as pure functions). None of them knows `BenchApp`.
- **The app** (`src/`): `main.rs`, `app.rs`; `app/` what belongs to the whole app (`model.rs`,
  `places.rs` the one answer to "where", `agents.rs` the links to agents, `events.rs`, `keys.rs`,
  `menu.rs`, `armed.rs`, `subscriptions.rs`…); the window by area (`workspace/`, `titlebar/`,
  `sidebar/`, `thread/`, `settings/`); by feature (`inserts/`, `env/`, `explorer/`); services that
  draw nothing (`client/`, `media/`, `system/`); `demo/` the fake agent, behind `client/fake.rs`.
- **The agent** (`agent/src/`): `server.ts` collects `modules/*.ts` (each declares its RPC methods
  and REST routes); `runtime/` the threads' core (`runtime.ts`, each part a file reached as
  `rt.<part>`, `tools-host.ts` the tools' host and `envFor`, `journal.ts` Pi's journal as the items
  Bench shows); `extensions.ts`, `extensions/assistant.ts`, `prompt.ts` Pi's extensions and prompt
  sections; `builtin/` the agent's own tools and stores; `turn/` what the hooks keep of a turn (step
  limit, window, frozen prompt, compaction); `judge/` the evaluator; `machine/` where tools work
  (over SSH, kept in a folder); `mcp/` MCP servers and sign-in; `lsp/` language servers.

The rules: a crate depends only on crates below it; pixel values and spacing live only in
`crates/ui`; "where" is answered only by `src/app/places.rs`; a new insert is a folder in
`src/inserts/`, a new environment section a file in `src/env/`, a new area of the agent a module in
`agent/src/modules/`; product code does not know the demo.
