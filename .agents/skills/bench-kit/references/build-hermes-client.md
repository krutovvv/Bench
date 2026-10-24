# Hermes GPUI ↔ the Hermes gateway

Sources of truth: the gateway is `~/.hermes/hermes-agent` (Python; `hermes_cli/web_routers/*`,
`hermes_cli/dashboard_auth/native_http.py`, `gateway/session_authority.py`), and Hermes Desktop
(`apps/desktop/electron/*.ts`) is the reference client — when unsure, do what Desktop does.
Answer protocol questions with the app's `--probe` mode or a temporary probe, not by clicking.

## Authentication (local gateway)

`connection.rs`: `hermes gateway ensure --json` gives the endpoint; a long-lived Python helper
(`hermes_cli.gateway_client._session_ticket`) mints **one-shot** tickets, sent as the
`X-Hermes-Gateway-Ticket` header (REST) or a WebSocket subprotocol. Over SSH the app runs its own
`hermes serve` and uses a session token instead (no profile/purpose rules).

A ticket is bound to **one profile** (`profile_id` = `~/.hermes` or `~/.hermes/profiles/<name>`)
and **one purpose** (`gateway/runtime_bootstrap.py`):

| Purpose | For |
|---|---|
| `native-http` | every REST route, scoped to that profile |
| `native-profile-create` | `POST /api/profiles` (any served profile may mint it) |
| `native-profile-registry` | `PATCH`/`DELETE /api/profiles/<name>` — minted **for that `<name>`**; never the gateway's launch home (except PATCH of default = display name) |
| `interactive` | the WebSocket (scope `host`: all served profiles) |

`rest.rs::purpose(method, path)` mirrors Desktop's `nativeGrantPurpose`; `Connection::rest_auth(profile, purpose)`.

Scope rules that bit us (`native_http.py::_check_profile_scope`):
- `/api/profiles/<name>/…` (soul, model, description…) needs a ticket **of `<name>`**, not of the
  status bar's profile → 403 `profile_scope_mismatch` otherwise. Pass the target profile as the
  call's `profile` argument (the `?profile=` query must match it too; `rest::url` adds it).
- Any `profile` in the query or JSON body must equal the ticket's profile.
- A just-created profile is not served for ~250 ms: minting fails with
  `invalid_control_response`. `rest::create_profile` waits until the new profile answers.
- Test profile admin with a throwaway profile (create → soul → save → rename → delete) and say so.

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
- Fenced mutations (model, reasoning, rewind, delete) send `expected_revision` and
  `expected_generation` (`Rpc::mutate`).
- Errors of fire-and-forget calls go to `~/Library/Application Support/hermes-gpui/gateway-errors.log`
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

`hermes gateway restart` refuses while work is live (turns, cron, delegations) and, without a
terminal, cannot ask: exit 1, the reason on **stdout** (`✗ Refusing to restart the gateway: N
live session(s) would be interrupted.`). The app runs it with `--detach`, shows that reason, and a
second press within a minute adds `--yes` (`connection::restart_gateway`, `Restart::Busy`).
Never restart the user's gateway yourself while a turn runs — it kills their work.

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
  to the composer (`rest.rs` `transcribe`). The STT engine is Hermes' (`stt.openai.base_url /
  model / language` in the profile's config — editable in Bench's Providers).
- **Send while recording** = stop + transcribe + send in one press: `send()` sets
  `send_after_transcribe = true` and stops the mic; the transcription's callback sends. Esc
  cancels a recording.
- **Read aloud**: TTS through Hermes, played with `rodio` (features `symphonia-mp3 … -aac`).

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
- Only the person's own chats (`source_label == "Hermes"`): Telegram, cron and API sessions run
  all the time and would be noise. Nothing when the finished chat is the one on screen in front.
- **Only inside the .app.** Asking for the notification center from a bare `target/` binary
  aborts the process (no bundle): `notify` checks `current_exe()` contains `.app/Contents/MacOS/`
  and does nothing otherwise. Test it from the bundle (`shot.sh dist/<App>.app … DEBUG_NOTIFY=1`
  with a hook that posts one); the permission prompt shows on the person's screen the first time
  — tell them to allow it.

## Installing Hermes from the app

No `~/.local/bin/hermes` → the bottom button reads **Install Hermes Agent** (download icon) and
the composer is closed ("Install Hermes Agent to start a chat"). The button opens a local shell
in the chat area (the SSH mode's terminal, `term.rs` + `ui/terminal.rs`) and types
`bash -o pipefail -c 'curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash -s --
--skip-setup' && exit` — the person watches the log; success closes the shell and the app
connects; a failure leaves it open with "Try again". Bench `src/app/install.rs` (`SetupKind`),
`connection::INSTALL_HERMES`, `hermes_installed()`. Can't be tested on a Mac that has Hermes:
say so, don't uninstall theirs.

## Delivered files

- Agents deliver files as `MEDIA:/abs/path` lines (Telegram/Desktop convention). Rendering:
  `references/build-media.md`.
- Bytes: `GET /api/fs/download?path=…` (raw, streamed; `native-http` ticket of the session's
  profile), or `/api/fs/read-data-url` (base64, size-capped). A local gateway's files are read in
  place.
- Old sessions often point at deleted files (`/tmp/…`, cleaned Desktop): show a quiet error row.

## Sending files

- `prompt.submit` (canonical) takes `attachments: [{path, mime}]`: **images only** (png, jpeg,
  gif, webp; ≤ 10), and each path must sit directly in the profile's image cache — stage it with
  `POST /api/chat/image-upload {data_url, filename}` (returns `path`), which works for local and
  SSH gateways alike. The model then sees the image itself.
- Any file (images too) is also named in the text as `[attached: <path on the gateway's
  machine>]`: a local file in place; a temporary one (`/var/folders`, `/tmp`, pasted) copied to
  `<profile home>/attachments/`; over SSH uploaded there with `POST /api/files/upload
  {path, data_url}` (after `/api/files/mkdir`). The UI turns those lines back into chips
  (`attach::split`). Non-canonical (SSH token) sessions get only the text lines.
- Verified headless: staging and copies, without submitting a turn. Clean up probe files.

## Profiles in the UI

- Status bar profile = the one chats run in and new profiles are cloned from.
- Settings → Profiles may open and edit ANY profile's SOUL.md; the call uses that profile's ticket.
- The status bar shows approvals / reasoning / model only after `profile_defaults` arrived
  (`profile_ready`), never placeholder defaults.

## Bench: SSH mode and the mini IDE

The reference app is **Bench** (`dist/Bench.app`, bundle id `local.hermes-gpui`; a Bench made by
`new-bench.sh` has `local.<crate>` — use that in the commands below). Besides
sessions it has an SSH mode (servers = `~/.ssh/config`, a terminal per server, no agent
installed on servers; gateways list = This Mac + config hosts where Hermes answers) and a mini
IDE (gateway files, or a server's files as synced cache copies under
`~/Library/Caches/hermes-gpui/remote/` — "Add to chat" hands the agent that path; uploads check
the server's mtime and stop on a conflict). Local agents reach servers through the separate
`~/Projects/ssh-mcp` MCP server (same `~/.ssh/config`, same `~/.ssh/cm` socket).

## Shipping changes to the user

The user runs a build from `dist/` (pinned in the Dock). **Builds are a carousel, one app per commit** (Bench is developed from inside Bench): commit,
then `scripts/bundle-macos.sh` makes `dist/Bench.<commit>.app` (7 characters of the hash) as a
NEW app next to the others, and `ship.sh` opens it beside the running one. **Never quit, kill or
replace the app the person runs** — it is the window they work in and talk to you through. They
try the new build and move the Dock icon to it when it's good; the newest 6 builds stay for
rolling back. Uncommitted changes stop the build (`BENCH_DIRTY=1` for a throwaway one). Saying "done" without
building the app got "I see no changes".
