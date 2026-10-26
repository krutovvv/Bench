# How Bench is put together

The whole picture — what Bench is, where it will grow, and the structure that takes that growth —
is `docs/architecture.md` in the repository. Where each thing is: `docs/map.md`. In short:

- **Crates** (`crates/`): `ui` (every element and size), `storybook` (all of them on one page),
  `protocol` (what Bench and the agent say about a thread; TypeScript generated for the agent),
  `thread` (the thread as rows, from Pi's journal, with the rules of order as tests),
  `devtools` (the /tools utilities as pure functions). None of them knows `BenchApp`.
- **The app** (`src/`): `app.rs` and `app/` (the window's state and actions), `ui/` (the frame:
  sidebar, header, status bar, composer, the thread view in `ui/thread/`), `inserts/` (one folder
  per insert kind), `env/` (a project's environment, one file per section), `gateway.rs` and
  `rest.rs` (the protocol to the agent).
- **The agent** (`agent/src/`): `server.ts` collects `modules/*.ts` (each declares its RPC methods
  and REST routes); `runtime.ts` runs Pi conversations; `thread.ts` turns Pi's journal into the
  items Bench shows; `builtin/` are the agent's own tools.

The rules: a crate depends only on crates below it; pixel values live only in `crates/ui`; a new
insert is a folder in `src/inserts/`, a new environment section a file in `src/env/`, a new area
of the agent a module in `agent/src/modules/`.
