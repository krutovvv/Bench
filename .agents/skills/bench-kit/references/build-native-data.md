# Where a tool's data lives

A tool's records must be reachable by the agent with Bench closed — in Telegram, on a schedule,
from another machine — and readable by Bench. So they never live only in Bench's memory or in
Bench's view state. Two places, in this order:

1. **Where they already live**, when that place is the person's long-lived one: servers in
   `~/.ssh/config`, files in their folders and on their hosts, sign-ins in the browser's profile,
   a service's records behind its API or MCP server. Bench shows them there; the agent reaches
   them with its own tool or MCP server.
2. **`~/.bench/bench.sqlite`**, Bench's one home, owned by the agent: tasks, hosts (SSH and
   FTP/SFTP accesses), settings, providers, sessions, cron jobs. Bench reads and writes it
   **through the agent's REST API** (`/api/tasks`, `/api/hosts`, routes in
   `agent/src/server.ts`), never by opening the file; the agent through its tool
   (`agent/src/builtin/tasks.ts`, `hosts.ts`). One store, two clients.

Not: a JSON or SQLite file of Bench's own, a third-party app as the store, a database inside
Bench. Passwords for hosts are kept in `bench.sqlite` as the person chose (plain text is fine for
them); a "from Keychain" button can fill one in (`security find-internet-password`,
`src/app/remote_files.rs`).

## Who else writes there

The agent (its tool), cron jobs, Telegram, another Bench window, another machine. So Bench
re-reads the list now and then and never assumes it is the only writer (`build-insert.md`, "a
list re-read from its source": keep unsaved new items, don't overwrite a card being typed into).

## Bench's Tasks, the worked example

- **The data**: `Task` in `agent/src/builtin/tasks.ts` (title, notes, tags, day, evening,
  someday, important, completed, spent seconds, a running timer) in `bench.sqlite`. The same
  shape in Rust: `src/tasks.rs` (lists: Today, Upcoming, Anytime, Logbook — someday tasks show in Anytime; pure
  functions, tested).
- **Bench's side**: `src/task_store.rs` — a worker thread that sends `Cmd::Load / Save / Delete`
  as REST calls and returns `Evt::Loaded / Created / Error` over an `async_channel`; the `/tasks`
  insert shows them (`src/ui/inserts/tasks.rs`, `src/app/tasks.rs`, `src/ui/tasks.rs`).
- **The agent's side**: the `tasks` tool (list with status, tag, `done_since` / `done_until`,
  sums `spent_minutes_total`; add; update; complete; delete). Its line in the `inserts` prompt
  section says
  "the only place they live, never look for them in files".
- **How they got there**: years in Things, then Apple Reminders (1318 tasks), then
  `bench.sqlite` (2026-10-24): Bench read every reminder once over EventKit
  (`move_from_reminders`, `src/task_store.rs`), wrote a backup to `~/.bench/ui`, imported them
  through `POST /api/tasks/import`, and left Reminders untouched. A marker file makes it one-off.

## Moving data in, and everything that used the old place

The import is the easy half. Then find **everything that read the old place** and move it too,
or it breaks silently weeks later:
- the agent's skills (`~/.bench/agents/<agent>/skills/`) that name the old app or its tools;
- its memory and standing instructions (`memories/`, `SYSTEM.md`, `AGENTS.md`);
- **scheduled jobs** (the `cron` tool's jobs in `bench.sqlite`): rewrite each prompt for the new
  tool; fix date conventions the old tool needed (an exclusive "to" vs an inclusive one);
- an MCP server that served the old place: remove it from the agent's `mcp.json`, or the agent
  keeps two ways to the same data and picks the stale one.
Back up first; check the import by counts (open / done / with time / flagged) on both sides,
a few spot checks by title, and in Bench itself. Check each changed job by running its prompt
once in a thread, with nothing delivered anywhere.

Mapping that worked for tasks (Things → Reminders → bench.sqlite): title → title (a trailing
"- 2 часа" moved to spent); notes → notes; tags → tags; a start date / Today → the day; This
Evening → evening; Someday → someday; a deadline → the flag (that person used deadlines only for
it); completed → completed with its original date. Projects, areas and checklists were not used —
ask before dropping them for someone who uses them.

## Projects without a folder

The sidebar holds only projects. A project's folder (a codebase) is optional: without one its
threads work in a folder Bench keeps for it (`src/app/project_list.rs`). An "audit" or a
"research" of a few deep threads is such a project. (`connection::group_dir`, which makes
`~/Research/<name>` from "Name a group", is the older way and still in the code.)
