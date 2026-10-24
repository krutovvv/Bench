# The agent: Pi Durable behind Bench

Bench talks to one agent: `agent/` in the repo, a Node server on
[Pi Durable](https://github.com/earendil-works/pi). It replaced the former agent on
2026-10-24; that one's name survives only in a few wire identifiers and in `~/.hermes`, read
once on import. The person's rule for it: **build on Pi as its authors describe it, extend with
their packages or popular community ones, write as little of our own as possible.** `agent/README.md`
lists what is Pi's, what is a package and what is ours (~3k lines).

## Shape

- **Protocol** (ours, inherited from the former agent's "token mode", unchanged): JSON-RPC 2.0 over
  `ws://…/api/ws?token=`, REST under `/api/` with `X-Hermes-Session-Token` (the header keeps its
  name). Bench's client code speaks it in `src/gateway.rs` / `src/rest.rs`.
- **Start**: `src/pi_agent.rs`. On this Mac `ensure()` copies the sources the app carries
  (`Contents/Resources/agent`, or the repo's `agent/` for a dev build) to `~/.bench/app`, runs
  `npm ci` when `package-lock.json` changed, starts `node src/server.ts`, reads `~/.bench/server.json`.
  Over SSH `ensure_remote(host)` ships the sources as a tar over ssh, installs, starts, and Bench
  tunnels to the port (`connection.rs`). A server built from other sources is restarted when idle.
- **Home** `~/.bench`: `bench.sqlite` (settings, keys, providers, sessions, cron jobs),
  `pi.sqlite`, `agents/<name>/` (Pi's agent directory: settings.json, mcp.json, SYSTEM.md, AGENTS.md, skills/, prompts/, extensions/; plus memories/), `extensions/`,
  `ui/` (Bench's own state). First start imports from `~/.hermes` once (`agent/src/import-hermes.ts`).
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

- `node agent/scripts/probe.ts "prompt"` — a Bench stand-in (`PROBE_PROFILE`, `--session <id>`,
  `PROBE_ANSWER` answers clarify/approval).
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

## Threads with a subject (2026-10-24): no modes, everything is a thread

The person's concept: one chat agent, Zed's Delta extended. The sidebar lists subjects — Tasks
(Today…Logbook), Servers (~/.ssh/config hosts and this Mac's terminal), Files (FTP/FTPS/SFTP
hosts from `/api/hosts`), Tools (one row; a picker strip in the thread) — and project threads.
A click opens the latest thread about it (or a new one): the subject's live block on top
(`src/app/subjects.rs`, `render_chat`), the conversation under it. The session's `subject`
(`tasks:today`, `ssh:<alias>` — `ssh:` alone is this Mac, `files:<host id>`, `tool:<name>`) goes
with `session.create`; the agent's prompt has a section about it (`prompt.ts` `subject()`).
With each message Bench sends `context` — what the block shows now (the terminal's last 60
lines, the host's folder and open file, the tool's input/output) — appended for the model inside
`<block-state>` and stripped from the history. The agent acts on the block with `tasks`,
`ssh_run`, `remote_files`. `!command` in any thread runs it in a terminal opened under that
thread (a server thread: in its terminal). Terminals fit their text (`used_rows`, up to 440).

## The server browser in threads (Browser group)

The browser threads are Bench's centrepiece (`build-subject.md`). The browser is a shared Chrome
on a server: `browser-server/` (Dockerfile, compose, set-up in `browser-server/README.md`) runs
Chrome, the Playwright MCP the agent drives, and the **`/live` gateway** for people
(`browser-server/live.mjs`), routed by Traefik at `https://<your browser host>/live`. No SSH:
each person has their own key (`docker compose exec browser node /opt/live.mjs add <name>`; only
its sha256 is stored; `remove`, `list`), kept in `~/.bench/ui/browser.json` (0600). The gateway
passes a narrow set of operations (tabs, watch, wake, open, close, mouse, wheel, text, keys) —
never raw CDP, which would expose cookies and code execution — and logs them
(`/data/live-audit.log`). The agent drives the browser through the existing MCP (key
`BROWSER_MCP_AUTH`); in a `browser:<id>` thread it opens a new tab each turn, and Bench gives every
tab opened during a turn to that turn (`src/app/sites.rs`).

**Kept light** (the person: the view is to show and let them step in, not to browse):
- ≤ 2 fps: the gateway acknowledges a screencast frame only after 500 ms; JPEG q55 ≤ 1100 px.
- Frames only while watched: Bench unwatches when no browser thread is open or nobody touched it
  for 60 s and the agent is idle ("paused — click to watch live").
- **Memory budget on the server** (a heavy tab is 200–450 MB of 1 GB): at most `LIVE_TABS` (3) awake
  tabs; the least recently touched unwatched one falls asleep (a screenshot + its address, the tab
  closed); `wake` reopens it (sign-ins live in the profile). Past `LIVE_HARD_TABS` (5) busy tabs,
  `open`/`wake` answer "busy" instead of the container dying. Ads and counters are blocked in every
  tab (auto-attach + `Network.setBlockedURLs`); Chrome runs without site-per-process.
- **Bench remembers what it saw**: each step's latest frame goes to
  `~/.bench/ui/sessions/<session>/frames/` every few seconds and is deleted with the session; a step
  whose tab is gone shows that frame (or "the page is closed — click to open it again").

**Scaling** past one container: a pool of browser containers behind the same gateway protocol,
a session pinned to one; one BrowserContext per person when sign-ins must differ.

