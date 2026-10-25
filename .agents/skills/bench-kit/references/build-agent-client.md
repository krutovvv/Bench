# Bench ↔ its agent: the protocol

Bench talks only to its own agent (`agent/`, `references/build-pi-backend.md`): JSON-RPC over a
WebSocket plus REST, with a session token. The source of truth for each
method and route is the agent's `agent/src/server.ts`. Answer protocol questions with the app's
`--probe` mode or `node agent/scripts/probe.ts`, not by clicking.

## Authentication

`connection.rs`: `pi_agent::ensure()` (this Mac) or `pi_agent::ensure_remote(host)` (over an
`ssh -L` tunnel) gives a port and a session token. The WebSocket opens
`ws://127.0.0.1:<port>/api/ws?token=<token>`; every REST call sends the token header that
`Connection::rest_auth` returns. There are no tickets or purposes in the auth. Which agent dir a
call is about (the Host's `default`, or a project's environment `project:<id>`) is its `profile`
argument (`?profile=`, `rest::url` adds it) — a wire name only; the UI has no profiles. A host's port and token are
saved in `state_dir()/ssh-<host>.json` to reattach next launch.

## Sessions over JSON-RPC (`gateway.rs`)

- Every event frame's `params` carries the execution stamp, incl. `execution_generation` of the
  turn; `session.info` payloads carry `revision` + `execution_generation`. Keep them per session
  (`Rpc::fences`).
- **Stop** = `session.interrupt {session_id, profile, execution_generation}`; without the current
  generation the gateway answers `4001 stale_generation` and the agent keeps running. Where the
  number comes from: every incoming frame — `if let Some(g) = msg["params"]["execution_generation"]
  .as_i64() { fences.entry(sid).or_insert((0,0)).1 = g; }` (Bench `gateway.rs`, the reader loop),
  plus `session.info`'s `revision`/`execution_generation` when a chat is opened.
- **The turn's clock** counts from the turn's real start, not from when you looked: a stored
  turn's start is its user message's timestamp (`history.rs`, `turn.started = meta.started`), a
  new one's is `now_secs()` at send; `live_since(started)` turns it into an `Instant`. So switching
  chats and coming back keeps the time running ("время хода не должно сбрасываться").
- Model, reasoning and retry are token-mode calls (`slash.exec` `/model …`, `config.set`
  `reasoning`, `session.undo` + `prompt.submit`); Stop and answers name the turn's
  `execution_generation` (`Rpc::fences`).
- Errors of fire-and-forget calls go to `~/.bench/ui/gateway-errors.log`
  — read it first when a button "does nothing".

## Questions and plans

- **Questions** (the `clarify` tool): event `clarify.request` {prompt_id, execution_generation,
  question, choices[], multi_select}; answer `clarify.respond` {session_id, execution_generation,
  prompt_id, answer} (multi-select: choices joined with ", "; empty answer = declined);
  `clarify.settled` closes it. The gateway times a question out (~10 min) — unanswered ones
  used to vanish into the step list; the app shows a card (choices, own answer, Decline).
- **Plan** (the `todo` tool): `todo.updated` {todos: [{id, content, status, parent?}], revision},
  also merged into the todo tool's `tool.complete`; statuses pending / in_progress / completed /
  cancelled. Shown as a foldable strip over the input ("PLAN · 2/5 · current step").
- **Reopening a chat**: `session.resume` returns `prompts` (open approvals and questions) and
  `todo_state`; the app replays them as events after the history (`Pending::Meta.replay`),
  otherwise a pending approval/question is lost when you switch chats.

## Gateway lifecycle

Restart (status bar) restarts the agent (`connection::restart_gateway` → `pi_agent::restart()`
on this Mac, the server's pid from `~/.bench/server.json` over SSH). Its runs are durable: an
unfinished answer continues after the start, so nothing is refused (`Restart::Busy` stays for a
backend that would refuse). Still, don't restart the user's agent yourself without a reason.

## Background processes (`process.list`, `process.kill`)

The agent starts long jobs in the background (servers, downloads, scripts). While a chat is
open, poll every 3 s: `process.list {session_id, profile}` → `{processes: [{session_id (the
process id!), command, status ("exited" or running), exit_code, started_epoch, exited_at,
completion_reason, output_tail | output_preview}]}`. Show them as a card under the last answer,
the same frame as Changed files: a folded line "PROCESSES · 2 running", rows with the command
(mono, truncated), a live clock or the exit code, **Stop** (`process.kill {session_id, process_id,
profile}`, then poll again), and a click that unfolds the output tail. Running first, then newest.
Bench `src/app/processes.rs`, `gateway.rs` `processes()`, `ui/chat.rs` `render_processes`.

## Voice: dictation and reading aloud

- **Dictate**: `cpal` records the default input as mono f32 (`audio.rs` `Recorder`), the level
  feeds the waveform; Stop → WAV bytes → `POST /api/audio/transcribe` `{data_url:
  "data:audio/wav;base64,…", mime_type: "audio/wav"}` (180 s timeout) → `{transcript}`, appended
  to the composer (`rest.rs` `transcribe`). The STT engine is the agent's (`stt.openai.base_url /
  model / language` in the agent's settings — editable in Bench's Providers).
- **Send while recording** = stop + transcribe + send in one press: `send()` sets
  `send_after_transcribe = true` and stops the mic; the transcription's callback sends. Esc
  cancels a recording.
- **Read aloud**: TTS through the agent, played with `rodio` (features `symphonia-mp3 … -aac`).

## "The agent finished": a macOS notification

Turns run for minutes to hours while the person is elsewhere. Bench `src/notify.rs`
(UserNotifications through `objc2-user-notifications`): `notify::request()` once at start (macOS
asks the person once), `notify::post(id, title, body)` — `id` = the session's id, so a newer
notification replaces the older one for the same chat.
- **The open chat** finishes (`Evt::Complete` for the current session) **while the window isn't
  active** (`!window.is_window_active()`): title = the chat's title, body = the answer's first
  line, clipped (`ui::clip`, character-safe).
- **Another chat** goes from `running` to not in a sessions refresh (`Evt::Sessions`: compare
  the old list with the new before replacing it): body "The agent finished".
- Only the person's own chats (`source_label == "Bench"`): Telegram, cron and API sessions run
  all the time and would be noise. Nothing when the finished chat is the one on screen in front.
- **Only inside the .app.** Asking for the notification center from a bare `target/` binary
  aborts the process (no bundle): `notify` checks `current_exe()` contains `.app/Contents/MacOS/`
  and does nothing otherwise. Test it from the bundle (`shot.sh dist/<App>.app … DEBUG_NOTIFY=1`
  with a hook that posts one); the permission prompt shows on the person's screen the first time
  — tell them to allow it.

## Delivered files

- Agents deliver files as `MEDIA:/abs/path` lines (Telegram/Desktop convention). Rendering:
  `references/build-media.md`.
- Bytes: `GET /api/fs/download?path=…` (raw, streamed; the session token), or `/api/fs/read-data-url` (base64, size-capped). A local gateway's files are read in
  place.
- Old sessions often point at deleted files (`/tmp/…`, cleaned Desktop): show a quiet error row.

## Sending files

- Every file (images too) is named in the text as `[attached: <path on the gateway's
  machine>]`: a local file in place; a temporary one (`/var/folders`, `/tmp`, pasted) copied to
  `~/.bench/attachments/`; over SSH uploaded there with `POST /api/files/upload
  {path, data_url}` (after `/api/files/mkdir`). The UI turns those lines back into chips
  (`attach::split`).
- Verified headless: staging and copies, without submitting a turn. Clean up probe files.

## Agent defaults in the status bar

- The status bar shows approvals / reasoning / model only after the agent's defaults arrived
  (`rest::profile_defaults`, the `profile_ready` flag), never placeholder defaults.
- A project's soul (SYSTEM.md), MCP servers, skills, prompts, accesses, Telegram bot and schedule
  are its environment, edited in the block a project's pencil opens (`app/project_env.rs`).

## Bench: servers and files

The reference app is **Bench** (`~/Applications/Bench.app`, bundle id `local.bench`; Bench Dev is `local.bench.dev`). Gateways are
This Mac plus `~/.ssh/config` hosts with Node 22.18+, where Bench installs the agent on connect;
a project with a remote host runs on that host's agent. Servers, file hosts and the project's
folder come into a thread as inserts (`/ssh`, `/ftp`, `/files`, `build-insert.md`); the agent
reaches them with `ssh_run` and `remote_files`, Bench with the agent's REST (`/api/hosts`,
`/api/fs/…`). A whole project opens in Zed from its ⋯ menu.

## Shipping changes to the user

**Two apps, Dev and release** (Bench is developed from inside Bench): the person works in
**Bench** (`~/Applications/Bench.app`, `local.bench`) and tries every commit in **Bench Dev**
(`dist/Bench Dev.app`, `local.bench.dev`). `ship.sh` quits a running Bench Dev (a normal quit),
rebuilds it in place and opens it. **Never quit, kill or replace the release app** — it is the
window the person works in and talks to you through. When Dev is good and the person asks,
`ship.sh --release` rebuilds Bench: the running one goes on from `Bench.prev.app` (kept for
rolling back) and the new one starts on the next open. Both are signed with the Apple Development
certificate, so macOS grants are given once per app. Uncommitted changes stop the build (`BENCH_DIRTY=1` for a throwaway one). Saying "done" without
building the app got "I see no changes".
