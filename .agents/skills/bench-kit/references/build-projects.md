# Projects, agents, machines: where a project lives

The second core, beside the thread. Design: `docs/remote-projects.md`. This page is how it is
built and how to grow it without patches.

## Three things, kept apart

| | What | Where it is in code |
|---|---|---|
| **Agent** | a process of `agent/` that runs threads: this Mac's (key `""`) or a server's (its ~/.ssh/config alias) | links: `src/app/agents.rs` (`AgentLink`, `conn_of`, `route`); the list of servers' agents: kept by the Mac's agent, `agent/src/builtin/servers.ts`, `/api/agents` |
| **Machine** | where a project's files and commands are: the agent's own, or a server this Mac's agent reaches over SSH | `agent/src/machine/ssh.ts` (`SshExecutionEnv`, Pi's `ExecutionEnv` over one OpenSSH ControlMaster connection) |
| **Project** | name, environment, its agent, its machine, its folder, kept in it or not | the agent's `projects` table (`agent/src/builtin/projects.ts`); in Bench `ProjectEntry` |

In Bench:

- `ProjectKey` (`src/app/model.rs`): a project's agent and its id there. `ProjectKey::HOST` is an
  agent's own environment (its Host project). As a string: the id for this Mac's projects,
  `<agent>:<id>` for a server's (sessions, marks and folds are keyed by it).
- `Location` (`src/app/model.rs`): agent, machine (None: the agent's own), folder;
  `machine_here()` — the machine as this Mac reaches it; `files_machine()` — what the agent is
  asked with (`/api/fs?machine=`) when it works on another machine.
- `src/app/places.rs` is the only place that says where things live: it puts `projects`
  together from their sources (`local_projects`, `server_agents` with each one's Host,
  `server_projects`) — `compose_projects` — and answers `location(project)`,
  `thread_location()`, `env_target`, `create_cmd`, `rename_project`. Everything else asks it.

## The agent side

- A conversation's environment: `Runtime.envFor` — the project's machine (`SshExecutionEnv` or
  `NodeExecutionEnv`), wrapped by `machine/confined.ts` when the project is kept in its folder. Pi's
  tools and ours (grep, glob, ls) work through it and never know where they are.
- `/api/fs/*` take `machine`: the same environment, so Bench's file views and a thread's media reach
  the project's machine through its agent (`agent/src/modules/files.ts`).
- Attachments of such a project go to its machine's `/tmp/bench-attachments/` (one-off, gone with
  a reboot): `attach::prepare` with `files_machine()`, `/api/files/upload` with `machine`.
- What the folder itself says — AGENTS.md / CLAUDE.md up to the repository's root, `.agents/skills`
  and `.claude/skills` — is read where the folder is: `agent/src/repo.ts` (one script over the
  connection, kept 30 s). Never `fs` a project's folder in the agent: it may be on a server.
- One server's connection carries at most six commands at once (`machine/ssh.ts` `Gate`: sshd's
  MaxSessions is 10); a command whose connection dropped says so in its output.
- What a machine lacks for a project's language servers (Node, a program) is the server's
  "failed: …" state before any starts (`lsp.ts` `lspNeeds`), red in the Language servers section.
- The prompt says where the work is (`prompt.ts` `soul`: the server, the folder, kept in it).
- A server's agent is installed and updated by Bench (`client/launch.rs` `ensure_remote`; an older
  idle one is replaced on connecting).

## Growing it

- **A new kind of place** (another machine kind, another kind of agent): a field on the
  project in the agent, its meaning in `Location`, its environment in `envFor`. Bench's screens
  ask `places.rs`, so they follow.
- **A new project field shown in its environment**: the agent's `projects` row + its route; a
  row in `src/env/place.rs` (where it lives) or its own section.
- Never decide "where" outside `places.rs`; never key a project by anything but `ProjectKey`'s
  string; never treat a server's agent as a project row.

## Checking it

- `agent/test/ssh-env.test.ts` runs Pi's own conformance suite for environments against
  `SshExecutionEnv` through an ssh stand-in; `BENCH_TEST_SSH_HOST=<alias>` runs it on a real
  server. `confined.test.ts`, `lsp.test.ts`, `search.test.ts` the same way.
- Your own Bench copy (`build-verification.md`) with a copy of `~/.bench` whose projects table
  names servers shows the migration and the sidebar; it connects to the real servers' agents as
  a second client.
