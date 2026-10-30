# The agent: Pi Durable behind Bench

Bench talks to one agent: `agent/` in the repo, a Node server on
[Pi Durable](https://github.com/earendil-works/pi). The person's rule for it: **build on Pi as its authors describe it, extend with
their packages or popular community ones, write as little of our own as possible.** `agent/README.md`
lists what is Pi's, what is a package and what is ours.

## Shape

- **Protocol** (ours, `crates/protocol`): JSON-RPC 2.0 over `ws://…/api/ws?token=`, REST under
  `/api/` with the token in a header (`Connection::auth`, `src/client/connection.rs`; checked in
  `server.ts`). Bench's client code speaks it in `src/client/rpc/` / `src/client/rest/`. A REST call
  about one agent's settings says so with `?agent=` (`with_settings`, `agentOf`).
- **Start**: `src/client/launch.rs` `ensure(target)`, the same for this Mac and a server: the
  sources the app carries (`Contents/Resources/agent`, or the repo's `agent/` for a dev build) go
  as a tar to `~/.bench/app` there (a local shell or SSH), `npm ci` when `package-lock.json`
  changed, `.build` their hash; then `service.rs` makes the machine's service (launchd
  `local.bench.agent`, systemd `bench-agent`) run them — restarted when it ran other sources —
  and prints `~/.bench/server.json`. Bench tunnels to a server's port (`connection.rs`). The agent
  takes `~/.bench/agent.pid` at start (`agent/src/instance.ts`): a second one on the home exits.
  A copy of the home (`BENCH_HOME`) gets no service: start its agent by hand and stop it after.
- **Home** `~/.bench` (`config.ts`):
  - `bench.sqlite` — settings (among them `agents`, the servers' agents, and `presets`), keys,
    providers, the thread index and its search, cron jobs, projects; `pi.sqlite` — Pi's durable store;
  - `agents/default/` — the Host project's agent, the machine's own (Pi's agent directory:
    settings.json, mcp.json, SYSTEM.md, AGENTS.md, skills/, prompts/, extensions/; plus memories/);
  - `projects/<id>/` — a project's environment (wire name `project:<id>`, `agentDir` in
    `config.ts`: SYSTEM.md, mcp.json, lsp.json, skills/, prompts/ on top of the default agent's
    memory and settings);
  - `ssh/` (the ControlMaster sockets, `machine/ssh.ts`), `lsp/` (language servers installed here,
    `lsp/lsp.ts`), `mcp-oauth/` (MCP tokens), `backups/` (before a migration), `ui/` (Bench's own state),
    `attachments/`, `server.json`, `app/` (the installed server).
- **Extensions** are the unit of behaviour (Pi's term; not "harness" — in Pi that word is the
  whole runtime). Built in: `assistant`, `codemode`, `tool-search`, `mcp:<agent>`; each agent's own
  in `agents/<agent>/extensions/*.ts` (installed as `<agent>:<name>`), hot-reloaded. An agent picks
  them in `settings.json` `"extensions"`; `forAgent` (`extensions.ts`) always adds `codemode`, and
  `tool-search` where some of its tools are `deferred`. MCP servers are per agent (`mcp.json`, Pi's
  format and `exposure`), connected with the agent's first thread.

## Rules that came from the person

- **Lean prompt, no "discipline" blocks.** A local Qwen at 60–150 tok/s pays for every line and
  for every extra verification round. The assistant's prompt is seven short sections (below),
  ~6k tokens with 60 skills.
- **No automatic memory.** `memory` and `skill_manage` write only when asked; no nudges, no
  background review.
- **Telegram: two messages per turn.** A silent status message with the steps, edited in place,
  folded to "Done · N steps · T s"; the answer as its own message, so the notification means "done".
- **Popular packages only**: grammY, croner, the official MCP servers of Tavily/Brave for web
  search. `pi-durable-telegram` (39 downloads a month) was not taken. Pin versions ≥ 2 weeks old.

## Verify without a window

- `PROBE_MATCH=<title words> PROBE_SEND="prompt" ./target/debug/bench --probe local` — a turn
  through Bench's own client code (`src/client/probe.rs`; `PROBE_GET=/api/a,/api/b` prints REST
  answers, `PROBE_COMMAND`, `PROBE_SEARCH`).
- `cd agent && node --test test/*.test.ts`. `test/process.test.ts` fails while a leftover `sleep 3032`
  from an earlier run is alive: kill it.
- `BENCH_AGENT_TRACE=<file>` dumps Pi's raw events; `BENCH_AGENT_NO_MCP=1` skips MCP servers.
- Durability: `kill -9` the server mid-answer, start it again, the answer finishes; a cron job
  keeps its schedule across a kill.

## Gotchas met

- **Node's type stripping** rejects parameter properties (`constructor(private x)`), enums, namespaces.
- **Pi's first tokens come in `message_start`**, some answers whole in `message_end`: seed and
  flush per-block text, emit only the new suffix as `message.delta`.
- **Context used = input + cacheRead + cacheWrite** in pi-ai's usage.
- **Pi conversation ids are integers**: store as integer text, convert both ways.
- **Register every stored session's model before `harness.resume()`**, or resumed runs fail
  with `no_model`; then watch sessions with live tasks so their answers reach the index.
- **A cron job's sleeping task is aborted and re-created on every change** (`task_id` on the job),
  or an update leaves two tasks firing.
- **MCP blows the context**: one server with 170 tools is 150k tokens. So a server's tools are
  code mode's by default (`exposure: codemode`, `agent/src/mcp/mcp.ts`): no declaration reaches the
  model. `direct` and `deferred` are per server or per tool, by choice.
- **Screenshots go stale when the person is away** (display asleep or locked): the window does not
  repaint. Verify with the probes.
- **Editing `agent/` while a dev Bench runs** changes the build hash: each Bench start restarts the
  server from the half-edited copy.

## How each tool uses Pi Durable

Each piece is Pi's own mechanism used as documented; only the glue is ours.

- **Inserts** (`agent/src/runtime/inserts.ts`, `build-insert.md`): an entry `bench.insert` {id, key}
  written into the transcript where the person put it (a write, not a turn: never queued, lands
  mid-run), and the conversation's document `bench.inserts` ({items: {id: {key, state, folded,
  removed, after?}}}) changed in commits (`thread.insert`, `thread.insert.patch` in `modules/threads.ts`, told
  as `ThreadEvent::Inserts`). The model learns them from the `inserts` prompt section, which reads the
  document with `input.read.snapshot(Inserts, conversationId)` (`extensions/assistant.ts`), and
  what each shows now from the `<block-state>` Bench appends to each message (out of the thread
  Bench shows, `runtime/journal.ts` `stripBlock`; the model's history keeps it). Where a project's settings live is a line of the `environment` section.
- **Prompt sections** (`prompt.ts`, listed in `extensions/assistant.ts`, Pi's `section()`): soul,
  environment, inserts, project, skills, notes. All but inserts are held as first read until the next
  compaction (`turn/frozen.ts`): a changed section would rewrite the prompt's head and the model
  server's cache with it. Keep sections stable (no clocks, no counters) and put what changes every
  message into `<block-state>`.
- **The message queue** is Pi's inbox: a message sent while the agent works is submitted with
  `whenBusy: "followUp"` (sent when it answers) or `"steer"` (joins the run after its current
  tools), `runtime/runtime.ts` `submit`, with Bench's `request_id` as Pi's `requestId`; Bench gets it from
  `InboxDoc` in each commit (`thread.queue`), each row named with its request from Pi's submission
  records, and the placed entry with its submission and request (`runtime/journal.ts` `Requests`). Taking
  one back is `harness.abortSubmission`, which settles the submission and takes its item out of the
  inbox in one commit (`runtime/queue.ts`: `withdraw`, and `sendNow`, which re-submits it as a steer).
- **Cron** (`builtin/cron.ts`): each enabled job is a durable `defineTask` (`bench.cron`) that
  `sleepUntil`s its next run, so it survives restarts and a run missed while down happens on
  start; a `memo` per firing keeps a restart mid-run from running it twice. Jobs are rows in
  `bench.sqlite` with the task's `task_id`; a change aborts the task and arms a new one (`arm`).
- **ssh_run** (`builtin/ssh.ts`): a plain `defineTool`. Its data is where it lives
  (`~/.ssh/config`). Without an explicit `host` argument it uses what the thread has inserted
  (`threadOf(id).inserts`: its last terminal).
- **Browser**: the Playwright MCP on the browser server, an ordinary MCP server in the agent's
  `mcp.json` (`url` + `Authorization: ${BROWSER_MCP_AUTH}`, browser-server-mcp's README), connected
  through `@earendil-works/pi-mcp`. One tab per thread: the prompt line says to work only in the
  thread's tab (its address comes in `<block-state>`), and Bench gives the tab a turn opened to
  that thread (`src/inserts/browser/state.rs`).
- **MCP** (`mcp/mcp.ts`, `mcp/SPEC.md`): per agent dir, `mcp.json` in Pi's format, via
  `@earendil-works/pi-mcp` (stdio or streamable HTTP); tools named `mcp__<server>__<tool>`; `exposure`
  `codemode` by default, `direct`, `deferred` (behind `tool_search`), `hidden`. OAuth for HTTP servers that
  ask for it: `mcp/oauth.ts` (pi-mcp's oauth: discovery, PKCE, registration, refresh; tokens in
  `~/.bench/mcp-oauth/`, callback on `localhost:47823`).
- **Telegram** (`telegram.ts`, grammY): the general bot (Host) and one per project that has its own
  (`projects.telegram`); a project's bot runs with `project:<id>` and its chats are that project's
  threads — the same Pi conversations and runtime as Bench's threads (source `telegram`).
  `syncTelegram` starts and stops bots as settings change. Where Telegram is reachable only
  through a proxy: `TELEGRAM_PROXY`, else `HTTPS_PROXY`, in the agent's env.
- **Where tools work** (`runtime/tools-host.ts` `envFor`): Pi's `ExecutionEnv` per conversation —
  `NodeExecutionEnv` here, `SshExecutionEnv` (`machine/ssh.ts`, one OpenSSH ControlMaster connection
  per server) on a project's machine, wrapped by `machine/confined.ts` when the project is kept in its
  folder (paths outside refused; commands run in a sandbox that writes only there and in the temp
  folders: `sandbox-exec` on macOS, bubblewrap on Linux when installed). Pi's read/write/edit/bash and ours work through it unchanged.
- **grep, glob, ls** (`builtin/search.ts`): code search without bash, through `api.env`.
- **Diagnostics** (`lsp/lsp.ts`, `builtin/diagnostics.ts`): the project's `lsp.json` names its language
  servers (presets in `lsp/presets.ts`); a `ToolTask` `afterTool` hook adds the edited
  file's errors to `edit` and `write` results; a `diagnostics` tool asks for them.
- **Approvals** (`builtin/approvals.ts`): with approvals set to manual, a `beforeTool` hook asks the
  person (`host().approve`) before a dangerous `bash` or `ssh_run` command; "deny" blocks it.
- **Compaction**: Pi's policy; a project's share of the window becomes a per-model override
  (`config.ts` `compactionPolicy`, read by Pi at every use through `runtime/models.ts`
  `compactionOverrides`); `turn/compaction.ts` (`beforeCompact`) summarizes with the same model,
  reasoning off.
- **Code mode, the evaluator**: `pi-durable-dev-rules/references/bench-on-pi.md` has code mode; `agent/src/judge/SPEC.md` the evaluator.
- **Presets and servers' agents**: `builtin/presets.ts` (setting `presets`: MCP and language
  servers, compaction shares, reasoning levels; `/api/presets`), `builtin/servers.ts` (setting
  `agents`: the servers' agents this Mac knows; `/api/agents`). Kept by this Mac's agent; a
  project on a server gets its presets from here.
- **Storage**: `pi.sqlite` is Pi's durable store (`openNodeSqliteStorage`: transcripts, documents,
  tasks, inbox); `bench.sqlite` (`config.ts` `db`) is ours: settings, keys, providers, the thread
  index (`store.ts`), cron jobs, projects.

## The server browser (the `/browser` insert)

The browser insert is Bench's centrepiece (`build-insert.md`). The browser is a shared Chrome
on a server: **browser-server-mcp**, a project of its own (`~/Projects/browser-server-mcp`:
Dockerfile, compose, set-up in its README). It is just an MCP server; it runs Chrome, the
Playwright MCP the agent drives, and the **`/live` gateway** for people (its `live.mjs`), which
Bench supports, routed by Traefik at `https://<your browser host>/live`. No SSH:
each person has their own key (`docker compose exec browser node /opt/live.mjs add <name>`; only
its sha256 is stored; `remove`, `list`), kept in `~/.bench/ui/browser.json` (0600). The gateway
passes a narrow set of operations (tabs, watch, wake, open, close, mouse, wheel, text, keys) —
never raw CDP, which would expose cookies and code execution — and logs them
(`/data/live-audit.log`). The agent drives the browser through the existing MCP (key
`BROWSER_MCP_AUTH`) in the thread's own tab, and the tab the agent opens or moves while it answers
becomes that thread's (`src/inserts/browser/state.rs`).

**Kept light** (the person: the view is to show and let them step in, not to browse):
- ≤ 2 fps: the gateway acknowledges a screencast frame only after 500 ms; JPEG q55 ≤ 1100 px.
- Frames only while watched: Bench unwatches when no thread with a browser insert is open or nobody touched it
  for 60 s and the agent is idle ("paused — click to watch live").
- **Memory budget on the server** (a heavy tab is 200–450 MB of 1 GB): at most `LIVE_TABS` (3) awake
  tabs; the least recently touched unwatched one falls asleep (a screenshot + its address, the tab
  closed); `wake` reopens it (sign-ins live in Chrome's user data). Past `LIVE_HARD_TABS` (5) busy tabs,
  `open`/`wake` answer "busy" instead of the container dying. Ads and counters are blocked in every
  tab (auto-attach + `Network.setBlockedURLs`); Chrome runs without site-per-process.
- **Bench remembers what it saw**: each step's latest frame goes to
  `~/.bench/ui/sessions/<thread>/frames/` every few seconds and is deleted with the thread; a step
  whose tab is gone shows that frame (or "the page is closed — click to open it again").

**Scaling** past one container: a pool of browser containers behind the same gateway protocol,
a thread pinned to one; one BrowserContext per person when sign-ins must differ.

