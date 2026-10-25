# The agent: Pi Durable behind Bench

Bench talks to one agent: `agent/` in the repo, a Node server on
[Pi Durable](https://github.com/earendil-works/pi). The person's rule for it: **build on Pi as its authors describe it, extend with
their packages or popular community ones, write as little of our own as possible.** `agent/README.md`
lists what is Pi's, what is a package and what is ours (~3k lines).

## Shape

- **Protocol** (ours): JSON-RPC 2.0 over `ws://…/api/ws?token=`, REST under `/api/` with the
  token in a header (`Connection::rest_auth`, `connection.rs`; checked in `server.ts`). Bench's
  client code speaks it in `src/gateway.rs` / `src/rest.rs`. Its calls still name the agent dir
  `profile` (a wire name only: there are no profiles in the UI).
- **Start**: `src/pi_agent.rs`. On this Mac `ensure()` copies the sources the app carries
  (`Contents/Resources/agent`, or the repo's `agent/` for a dev build) to `~/.bench/app`, runs
  `npm ci` when `package-lock.json` changed, starts `node src/server.ts`, reads `~/.bench/server.json`.
  Over SSH `ensure_remote(host)` ships the sources as a tar over ssh, installs, starts, and Bench
  tunnels to the port (`connection.rs`). A server built from other sources is restarted when idle.
- **Home** `~/.bench`: `bench.sqlite` (settings, keys, providers, the session index, cron jobs,
  tasks, hosts, projects), `pi.sqlite` (Pi's durable store), `agents/default/` — the Host
  project's agent, the machine's own (Pi's agent directory: settings.json, mcp.json, SYSTEM.md,
  AGENTS.md, skills/, prompts/, extensions/; plus memories/) — and `projects/<id>/`, a project's
  environment (wire name `project:<id>`, `agentDir` in `config.ts`: its SYSTEM.md, mcp.json,
  skills/, prompts/ on top of the default agent's memory, model and settings), `ui/` (Bench's own state).
- **Extensions** are the unit of behaviour (Pi's term; not "harness" — in Pi that word is the
  whole runtime). Built in: `assistant`, `mcp`, `tool-search`; each agent's own in
  `agents/<agent>/extensions/*.ts` (installed as `<agent>:<name>`), hot-reloaded. An agent picks
  them in `settings.json` `"extensions"`. MCP servers are per agent (`mcp.json`, Pi's format and
  `exposure`), connected with the agent's first session.

## Rules that came from the person

- **Lean prompt, no "discipline" blocks.** A local Qwen at 60–150 tok/s pays for every line and
  for every extra verification round. The assistant's prompt is five sections (SOUL, environment,
  project AGENTS.md, skills list, notes), ~6k tokens with 60 skills.
- **No automatic memory.** `memory` and `skill_manage` write only when asked; no nudges, no
  background review.
- **Telegram: two messages per turn.** A silent status message with the steps, edited in place,
  folded to "Done · N steps · T s"; the answer as its own message, so the notification means "done".
- **Popular packages only**: grammY, croner, the official MCP servers of Tavily/Brave for web
  search. `pi-durable-telegram` (39 downloads a month) was not taken. Pin versions ≥ 2 weeks old.

## Verify without a window

- `node agent/scripts/probe.ts "prompt"` — a Bench stand-in (`PROBE_PROFILE` = the agent dir,
  `--session <id>`, `PROBE_ANSWER` answers clarify/approval).
- `PROBE_MATCH=<title words> PROBE_PROFILE=default PROBE_SEND="prompt" ./target/debug/bench --probe local`
  — a turn through Bench's own gateway code.
- `cd agent && node --test test/*.test.ts`.
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
- **A subagent's conversation has no session of its own**: `host().adopt(child, parent)`.
- **A cron job's sleeping task is aborted and re-created on every change** (`task_id` on the job),
  or an update leaves two tasks firing.
- **MCP blows the context**: one server with 170 tools is 150k tokens. Defer big servers
  (`deferred: true`) behind Pi's `tool_search`.
- **Screenshots go stale when the person is away** (display asleep or locked): the window does not
  repaint. Verify with the probes.
- **Editing `agent/` while a dev Bench runs** changes the build hash: each Bench start restarts the
  server from the half-edited copy.

## How each tool uses Pi Durable

Each piece is Pi's own mechanism used as documented; only the glue is ours.

- **Inserts** (`agent/src/inserts.ts`, `build-insert.md`): an entry `bench.insert` {id, key}
  written into the transcript where the person put it (a write, not a turn: never queued, lands
  mid-run), and the conversation's document `bench.inserts` ({items: {id: {key, state, folded,
  removed}}}) changed in commits (`session.insert`, `session.insert.patch` in `server.ts`, broadcast
  as `session.inserts`). The model learns them from the `inserts` prompt section, which reads the
  document with `input.read.snapshot(Inserts, conversationId)` (`extensions/assistant.ts`), and
  what each shows now from the `<block-state>` Bench appends to each message (stripped from the
  history, `runtime.ts` `stripBlock`). Old threads' `subject` is mapped to an insert
  (`legacyInsert`); the one subject left is `env:<id>`, a project's environment at a thread's top.
- **Prompt sections** (`prompt.ts`, Pi's `section()`): soul, environment, subject, inserts,
  project, skills, notes. Pi resends a section only when its text changed, so the provider's
  prompt cache stays warm: keep sections stable (no clocks, no counters) and put what changes
  every message into `<block-state>`.
- **The message queue** is Pi's inbox: a message sent while the agent works is submitted with
  `whenBusy: "followUp"` (sent when it answers) or `"steer"` (joins the run after its current
  tools), `runtime.ts` `submit`; Bench lists it from `InboxDoc` (`session.queue`). Taking one back
  is `harness.abortSubmission` + `dropFromInbox`: Pi 1.4 marks the submission unanswered but
  leaves the inbox item, so we remove it ourselves (`withdraw`, and `sendNow`, which re-submits it as a steer).
- **Cron** (`builtin/cron.ts`): each enabled job is a durable `defineTask` (`bench.cron`) that
  `sleepUntil`s its next run, so it survives restarts and a run missed while down happens on
  start; a `memo` per firing keeps a restart mid-run from running it twice. Jobs are rows in
  `bench.sqlite` with the task's `task_id`; a change aborts the task and arms a new one (`arm`).
- **Delegate** (`builtin/subagent.ts`): child conversations owned by the tool call's task
  (`createConversation({ownership: {kind: "task", taskId}})`), found again by `ownerTaskId` on a
  replay, so a rerun after a crash reuses them; up to 4 run in parallel, without `delegate`;
  `host().adopt(child, parent)` makes them act for the parent's session.
- **tasks, ssh_run, remote_files, hosts** (`builtin/*.ts`): plain `defineTool`s. Their data is in
  `bench.sqlite` (tasks, hosts with a `project` column) or where it lives (`~/.ssh/config`, the
  host's files); Bench reads the same through REST (`/api/tasks`, `/api/hosts`, …). Without an
  explicit `host` argument they use what the thread has inserted (`sessionOf(id).inserts`: the
  last terminal for `ssh_run`, the last file host for `remote_files`, `hostFor`).
- **Browser**: the Playwright MCP on the browser server, an ordinary MCP server in the agent's
  `mcp.json` (`url` + `Authorization: ${BROWSER_MCP_AUTH}`, browser-server-mcp's README), connected
  through `@earendil-works/pi-mcp`. One tab per thread: the prompt line says to work only in the
  thread's tab (its address comes in `<block-state>`), and Bench gives the tab a turn opened to
  that thread (`src/app/sites.rs`).
- **MCP** (`mcp.ts`): per agent dir, `mcp.json` in Pi's format, via `@earendil-works/pi-mcp`
  (stdio or streamable HTTP); tools named `mcp__<server>__<tool>`; `exposure` direct / deferred
  (behind `tool_search`) / hidden, big servers deferred by default. OAuth for HTTP servers that
  ask for it: `mcp-oauth.ts` (pi-mcp's oauth: discovery, PKCE, registration, refresh; tokens in
  `~/.bench/mcp-oauth/`, callback on `localhost:47823`).
- **Telegram** (`telegram.ts`, grammY): the general bot (Host) and one per project that has its own
  (`projects.telegram`); a project's bot runs with `project:<id>` and its chats are that project's
  threads — the same Pi conversations and runtime as Bench's threads (source `telegram`).
  `syncTelegram` starts and stops bots as settings change. Where Telegram is reachable only
  through a proxy: `TELEGRAM_PROXY`, else `HTTPS_PROXY`, in the agent's env.
- **Storage**: `pi.sqlite` is Pi's durable store (`openNodeSqliteStorage`: transcripts, documents,
  tasks, inbox); `bench.sqlite` (`config.ts` `db`) is ours: settings, keys, providers, the session
  index (`store.ts`), cron jobs, tasks, hosts, projects.

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
becomes that thread's (`src/app/sites.rs`).

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
  `~/.bench/ui/sessions/<session>/frames/` every few seconds and is deleted with the session; a step
  whose tab is gone shows that frame (or "the page is closed — click to open it again").

**Scaling** past one container: a pool of browser containers behind the same gateway protocol,
a session pinned to one; one BrowserContext per person when sign-ins must differ.

