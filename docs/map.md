# Where is what

One page to open before reading code. Find the row, open that file, not its neighbours.

## The common changes

| You are asked to… | Open |
|---|---|
| change a size, a gap, a font, a color, a button's look | `crates/ui/src/` (`metrics.rs`, `text.rs`, `button.rs`, `theme.rs`) — see it in `cargo run -p storybook` |
| add an icon | `assets/icons/<name>.svg`, a line in `crates/ui/src/icons.rs` |
| change how a message, an answer or "N actions" looks | `src/ui/thread/message.rs`, `src/ui/thread/work.rs` |
| change what the thread shows or in what order | `crates/thread/src/rows.rs` (the rules are tests in `tests.rs`) |
| change a queued message's row or its buttons | `src/ui/thread/queued.rs` (actions in `src/app/send.rs`: `queued`, `queue_edit_*`) |
| change the ticks on the right of the thread | `src/ui/thread/rail.rs` |
| change the composer | `src/ui/composer.rs`, sending in `src/app/send.rs` (`send`) |
| change an insert (`/terminal`, `/files`, `/ftp`, `/tasks`, `/tools`, `/browser`) | `src/inserts/<kind>/` — `mod.rs` the block, `state.rs` its data and actions, `view.rs` a long body |
| add an insert | `.agents/skills/bench-kit/references/build-insert.md` |
| change a project's environment (soul, MCP, skills, prompts, accesses, Telegram, schedule) | `src/env/<section>.rs` (`mcp.rs`, `skills.rs`, `schedule.rs`…); a new section: `src/env/mod.rs` header |
| change what happens on an agent event | `src/app/events.rs` |
| open, delete or search threads | `src/app/sessions.rs` |
| change the sidebar | `src/ui/sidebar.rs` (projects: `src/app/project_list.rs`); what is done to a thread or project there — pin, color, rename, delete, ↑ ↓ ⌫ — `src/app/sidebar_items.rs`, marks in `src/app/marks.rs` |
| change the status bar | `src/ui/statusbar.rs` |
| change the gear (providers, look) | `src/ui/providers.rs`, `src/app/providers.rs`, `src/ui/panels.rs` |
| change a keyboard shortcut | `src/app/keys.rs` |
| change a /tools utility | `crates/devtools/src/` (pure functions with tests) |
| change what Bench sends or receives | `src/gateway.rs` (WebSocket), `src/rest.rs` (REST); types shared with the agent in `crates/protocol` |
| change the agent's answer to an RPC or a REST route | `agent/src/modules/<area>.ts` |
| change how the agent runs a thread | `agent/src/runtime.ts`; what Bench reads of it: `agent/src/thread.ts` |
| change how work is done on a server (a project's machine over SSH) | `agent/src/ssh-env.ts` (Pi's `ExecutionEnv`; its tests are Pi's conformance suite, `test/ssh-env.test.ts`) |
| change the agents Bench talks to (this Mac's, servers') | `src/app/agents.rs`; the gear's list in `src/ui/providers.rs` |
| change an agent tool | `agent/src/builtin/<tool>.ts`, its prompt line in `agent/src/prompt.ts` |
| change the demo (screenshots) | `src/demo.rs` |

## The tree

```
crates/
  ui/            every element and size; nothing else has px values       (bench_ui)
  storybook/     all elements on one page; reference/ has the pictures
  protocol/      thread.open and thread.* types; writes agent/src/protocol.gen.ts
  thread/        the thread as rows from Pi's journal; rules of order as tests
  devtools/      the /tools utilities, pure functions
src/
  main.rs        start, the window
  app.rs         BenchApp: the window's state, its construction, the connection
  app/           its actions by area: agents (this Mac's and servers'), events, sessions, send, keys, slash, voice, projects, providers…
  ui/            the frame: mod.rs (layout, header), sidebar, statusbar, composer, media
  ui/thread/     the open thread on gpui::list: messages, work, queued, rail, extras
  inserts/       one folder per insert kind, the registry in mod.rs
  env/           a project's environment, one file per section, the registry in mod.rs
  gateway.rs     the WebSocket to the agent;  rest.rs  its REST API
  connection.rs, pi_agent.rs   where the agent runs, starting it (here or over SSH)
agent/src/
  server.ts      starts the agent, collects modules/
  modules/       threads, commands, projects (environment), hosts, tasks, cron, files, audio, settings
  runtime.ts     Pi conversations, the queue, inserts
  thread.ts      Pi's journal → the items Bench shows
  builtin/       the agent's tools
docs/            architecture.md (why), map.md (where), thread-rewrite.md (the thread's design),
                 remote-projects.md (agents, machines, projects on servers)
```

## Names worth knowing

- **Row** (`crates/thread`): one line of the thread — User, Work, Answer, Insert, Fail, Writing, Queued.
- **Work**: the agent's steps between two messages; counted per message, never per run.
- **Insert** / **InsertKind** (`src/inserts/mod.rs`): a live tool in the thread and its kind.
- **Section** (`src/env/section.rs`): one part of a project's environment.
- **Journal**: Pi's entries of a conversation; Bench's thread is read from it, nothing else.
