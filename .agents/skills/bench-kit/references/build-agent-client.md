# Bench ↔ its agent: the protocol

Bench talks only to its own agent (`agent/`, `references/build-pi-backend.md`): JSON-RPC over a
WebSocket plus REST, with a session token. The source of truth for each
method and route is the agent's module that declares it, `agent/src/modules/<area>.ts`
(collected by `server.ts`). Answer protocol questions with the app's
`--probe` mode or `node agent/scripts/probe.ts`, not by clicking.

## Authentication

`client/connection.rs`: `launch::ensure()` (this Mac) or `launch::ensure_remote(host)` (over an
`ssh -L` tunnel) gives a port and a session token. The WebSocket opens
`ws://127.0.0.1:<port>/api/ws?token=<token>`; every REST call sends the token header that
`Connection::rest_auth` returns. There are no tickets or purposes in the auth. Which agent dir a
call is about (the Host's `default`, or a project's environment `project:<id>`) is its `profile`
argument (`?profile=`, `rest::url` adds it) — a wire name only; the UI has no profiles. A host's port and token are
saved in `state_dir()/ssh-<host>.json` to reattach next launch.

## Which agent a command goes to (`src/app/agents.rs`)

Bench keeps a link to every agent (`AgentLink`: its gateway and REST connection), keyed `""` for
this Mac's and by the ~/.ssh/config alias for a server's. `route(cmd)` sends a command to the
agent it concerns — the open thread's (`thread_host()`), a session's own for delete and rename,
this Mac's for the sessions list; `conn_of(host)` and `thread_conn()` give the REST connection.
Never send to "the" agent: there are several.

## The thread (`thread.open`, `thread.*`)

- **Open**: `thread.open {session_id, since}` (`agent/src/modules/threads.ts`, `rt.thread`) →
  the journal's items after `since`, the queue, the inserts' state. `since` = the head Bench
  already has (a cached `Thread`, `thread_cache`), 0 for all of it.
- **Events**, from Pi's commits (`runtime.ts`): `thread.append {items}` (new journal items),
  `thread.stream {kind, delta}` (the answer or thinking being written; its entry's append
  replaces it), `thread.queue {items}` and `thread.inserts {items}` (whole, after every change),
  `thread.run {running}`. Parsed in `crates/protocol` (`ThreadEvent::parse`), applied by
  `crates/thread/src/state.rs` (`Thread::open`, `apply`). The rest: `build-thread.md`.

## Sessions over JSON-RPC (`client/rpc/`)

- Frames and `session.info` carry the run's `execution_generation`; the gateway keeps the last
  per session (`Rpc::fences: HashMap<String, i64>`, set by `note_fence`). **Stop** =
  `session.interrupt {session_id, profile, execution_generation}`; questions and approvals are
  answered with it too.
- **The work's clock** counts from its real start, not from when you looked: a `Row::Work` has
  `from`, `to` and `live` (`crates/thread`), drawn by `src/thread/work.rs` — a live one counts
  on, a finished one keeps the time its steps took. Switching threads and coming back keeps the
  time running.
- Model, reasoning and retry: `slash.exec` `/model …`, `config.set` `reasoning`,
  `session.undo` + `prompt.submit`.
- Errors of fire-and-forget calls go to `~/.bench/ui/gateway-errors.log`
  — read it first when a button "does nothing".

## Questions and plans

- **Questions** (the `clarify` tool): event `clarify.request` {prompt_id, execution_generation,
  question, choices[], multi_select}; answer `clarify.respond` {session_id, execution_generation,
  prompt_id, answer} (multi-select: choices joined with ", "; empty answer = declined);
  `clarify.settled` closes it. The agent times a question out (`clarify_timeout`, 600 s, `runtime.ts`) — unanswered ones
  used to vanish into the step list; the app shows a card (choices, own answer, Decline).
- **Plan** (the `todo` tool): `todo.updated` {todos: [{id, content, status, parent?}], revision},
  also merged into the todo tool's `tool.complete`; statuses pending / in_progress / completed /
  cancelled. Shown as a foldable strip over the input ("PLAN · 2/5 · current step").
- **Reopening a thread**: `session.resume` returns `prompts` (open approvals and questions) and
  `todo_state`; the gateway replays them as events once the thread is open
  (`Pending::Thread {replay}`), otherwise a pending approval/question is lost when you switch threads.

## The agent's lifecycle

Restart (status bar) restarts the agent (`connection::restart_gateway` → `launch::restart()`
on this Mac, the server's pid from `~/.bench/server.json` over SSH). Its runs are durable: an
unfinished answer continues after the start, so nothing is refused (`Restart::Busy` stays for a
backend that would refuse). Still, don't restart the user's agent yourself without a reason.

## Background processes (`process.list`, `process.kill`)

The agent starts long jobs in the background (servers, downloads, scripts). While a thread is
open, poll every 3 s: `process.list {session_id, profile}` → `{processes: [{session_id (the
process id!), command, status ("exited" or running), exit_code, started_epoch, exited_at,
completion_reason, output_tail | output_preview}]}`. Shown as a row of its own at the thread's tail,
beside Changed files: a folded line "PROCESSES · 2 running", rows with the command
(mono, truncated), a live clock or the exit code, **Stop** (`process.kill {session_id, process_id,
profile}`, then poll again), and a click that unfolds the output tail. Running first, then newest.
Bench `src/thread/processes.rs`, `src/client/rpc/events.rs` `processes()`, `render_processes` in the same `src/thread/processes.rs`.

## Voice: dictation and reading aloud

- **Dictate**: `cpal` records the default input as mono f32 (`audio.rs` `Recorder`), the level
  feeds the waveform; Stop → WAV bytes → `POST /api/audio/transcribe` `{data_url:
  "data:audio/wav;base64,…", mime_type: "audio/wav"}` (180 s timeout) → `{transcript}`, appended
  to the composer (`client/rest/` `transcribe`). The STT engine is the agent's (`stt.openai.base_url /
  model / language` in the agent's settings — editable in the gear's Agents tab, per agent).
- **Send while recording** = stop + transcribe + send in one press: `send()` sets
  `send_after_transcribe = true` and stops the mic; the transcription's callback sends. Esc
  cancels a recording.
- **Read aloud**: TTS through the agent, played with `rodio` (features `symphonia-mp3 … -aac`).

## "The agent finished": a macOS notification

Turns run for minutes to hours while the person is elsewhere. Bench `src/system/notify.rs`
(UserNotifications through `objc2-user-notifications`): `notify::request()` once at start (macOS
asks the person once), `notify::post(id, title, body)` — `id` = the session's id, so a newer
notification replaces the older one for the same thread.
- **The open thread** finishes (`Evt::Complete` for the current session) **while the window isn't
  active** (`!window.is_window_active()`): title = the thread's title, body = the answer's first
  line, clipped (`ui::clip`, character-safe).
- **Another thread** goes from `running` to not in a sessions refresh (`Evt::Sessions`: compare
  the old list with the new before replacing it): body "The agent finished".
- Only the person's own threads (`source_label == "Bench"`): Telegram, cron and API sessions run
  all the time and would be noise. Nothing when the finished thread is the one on screen in front.
- **Only inside the .app.** Asking for the notification center from a bare `target/` binary
  aborts the process (no bundle): `notify` checks `current_exe()` contains `.app/Contents/MacOS/`
  and does nothing otherwise. Test it in Bench after `ship.sh`; the permission prompt shows
  on the person's screen the first time — tell them to allow it.

## Delivered files

- Agents deliver files as `MEDIA:/abs/path` lines (Telegram/Desktop convention). Rendering:
  `references/build-media.md`.
- Bytes: `GET /api/fs/download?path=…` (raw, streamed; the session token), or `/api/fs/read-data-url` (base64, size-capped); with `machine` for a
  project worked on a server (`Location::files_machine()`). This Mac's agent's own files are read in place.
- Old sessions often point at deleted files (`/tmp/…`, cleaned Desktop): show a quiet error row.

## Sending files

- Every file (images too) is named in the text as `[attached: <path on the machine the work is
  on>]` (`attach::prepare`): a local file in place; a temporary one (`/var/folders`, `/tmp`,
  pasted) copied to `~/.bench/attachments/`; to a server's agent uploaded to its `attachments/`
  with `POST /api/files/upload {path, data_url}`; for a project worked on a server, to that
  machine's `/tmp/bench-attachments/` (the upload with `machine`). The UI turns those lines back
  into chips (`attach::split`).
- Verified headless: staging and copies, without submitting a turn. Clean up probe files.

## Agent defaults in the status bar

- The status bar shows approvals / reasoning / model only after the agent's defaults arrived
  (`rest::profile_defaults`, the `profile_ready` flag), never placeholder defaults.
- A project's soul (SYSTEM.md), MCP and language servers, skills, prompts, model,
  Telegram bot and schedule are its environment: its Settings page (`open_project_settings`,
  `src/env/mod.rs`), a section per file (`src/env/`), data from `/api/projects/<id>/env`.

## Agents, machines, projects

This Mac's agent always; a server's when the person adds it in the gear (Agents tab,
`src/settings/agent_form.rs`). The list of servers' agents is kept by this Mac's agent
(`agent/src/builtin/servers.ts`, setting `agents`, `/api/agents`); Bench installs and starts each
over SSH (`launch::ensure_remote`, Node 22.18+ on the server) and links to it
(`src/app/agents.rs`). A project = its agent + its machine (the agent's own, or a server this Mac's
agent works on over SSH) + an optional folder; `src/app/places.rs` answers where
(`build-projects.md`). The terminal on the project's machine comes into a thread as an insert
(`/terminal`, `build-insert.md`); the agent works there with `ssh_run` and its coding tools,
Bench reads the project's files with the agent's REST (`/api/fs/…`). A project opens in Zed from
its ⋯ menu.

## Shipping changes to the person

**One app, rebuilt in place, when the person asks for it** (not after every commit): `ship.sh`
quits the running Bench with `osascript … quit`, rebuilds `/Applications/Bench.app`
(`local.bench.app`) and opens it again; there is no other build. Bench is a client: threads, the
queue and the agent live in the agent process and `~/.bench`, so quitting and reopening it costs
the person a few seconds and an unfinished run comes back with the window. Never kill it or quit
it outside `ship.sh`. Rolling back means rebuilding an earlier commit — nothing is parked beside
`/Applications/Bench.app`. Signed with the Apple Development certificate, so macOS grants are
given once. Uncommitted changes stop the build.
Saying "done" without building the app got "I see no changes".
