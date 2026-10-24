# Data from native sources (the Bench keeps none of its own)

A Bench aggregates: the person's data stays where they already keep it and could open it
without the Bench (`start-why.md`). This page is how to do that, with Bench's Tasks mode (Apple
Reminders) as the worked example. Before building any mode with records, answer in order:

1. **Where does this kind of data already live on their Mac?** Tasks → Reminders; events →
   Calendar; people → Contacts; servers → `~/.ssh/config`; notes → files in a folder; the
   agent's sessions → Hermes. Use that. **Not** an own JSON/SQLite file and **not** a
   third-party app as the store, even when the person first suggests one ("я бы SQLite
   использовал"): propose the native place and why (it outlives the Bench, opens without it,
   syncs to their phone). The user rejected JSON, SQLite, Things and an MCP inside the Bench, in
   that order, before Reminders. Only if no native store fits at all: say so and ask.
2. **What has no field there?** Keep it as plain lines the store does have, readable anywhere:
   a reminder's notes end with `Tags: a, b` / `When: evening|someday` / `Spent: 1h 30m` /
   `Timer: <unix start>` (a running stopwatch); a line of only `#words` also counts as tags
   (how phones and agents tend to write them) — keep every one of these when you rewrite notes; a
   server's default folder is a comment in its `Host` block of `~/.ssh/config`. Parse them; write
   them back in the same form; leave everything else in the field untouched.
3. **Who else writes there?** The agents (Hermes profiles) do it directly with their own tool
   (an MCP server configured in the agent, never inside the Bench); the iPhone syncs. So the
   Bench re-reads (`build-modes.md`, section 8) and never assumes it is the only writer.

## Reading and writing Reminders (EventKit, in Rust)

Crates (in Bench's `Cargo.toml`, `build-crates.md`): `objc2 0.6`, `block2 0.6`,
`objc2-foundation 0.3`, `objc2-event-kit 0.3` with `default-features = false` and the EK
features used. Shape (Bench `src/reminders.rs`):
- **One worker thread owns the `EKEventStore`**; the UI sends it commands over a channel
  (`Cmd::Load { done_since }`, `Cmd::Save { task, token }`, `Cmd::Delete(id)`) and receives
  events (`Evt::Loaded(items)`, `Evt::Created { token, id }`, `Evt::Error(e)`) through an
  `async_channel` it awaits in `cx.spawn`. Never touch EventKit on the UI thread.
- Access: `requestFullAccessToRemindersWithCompletion`, wait for the block's answer (an mpsc
  channel). The .app's Info.plist needs `NSRemindersUsageDescription` and
  `NSRemindersFullAccessUsageDescription` (`scripts/bundle-macos.sh`), or macOS refuses silently.
- Fetch with `predicateForIncompleteRemindersWithDueDateStarting…` and
  `predicateForCompletedRemindersWithCompletionDateStarting…` (completed only since the range the
  list shows — 1300 done tasks read in milliseconds, but only fetch what is visible).
- Map: a day = date-only `dueDateComponents` (no time → no alarm); "important" = priority 1
  (high), 0 otherwise; **never add alarms** (the person's iPhone would ring); completion date is
  settable, so done tasks can be back-dated.
- Bind predicate objects to locals before passing references (`&unsafe { … }` temporaries don't
  type-check).
- A new item has a temporary id (`new:<token>`) until `Evt::Created` returns the real one.

**Never script Reminders through AppleScript/JXA**: reading 1400 reminders took 4 minutes.
EventKit does it in milliseconds.

## The agents' side: an MCP server in the agent, not in the Bench

The Bench is a view; the agents must work even when it is closed. Bench's example, **copied into
this kit at `templates/examples/reminders-mcp/`** (the live one is `~/Projects/reminders-mcp`):
copy the folder, `npm install`, `npm run build:helper` (compiles `helper/main.swift` with
`swiftc` into `bin/Reminders Bridge.app` and signs it ad hoc), `npm test`. Node (ESM, `@modelcontextprotocol/sdk`, `zod`), tools `list_tasks`
(status, from/to by completion date, tag, query), `get_task`, `list_tags`, `add_task`,
`update_task`, `complete_tasks` (ids, date, spent), `delete_task`; the same notes-line format
in `src/notes.js` as the Bench's `src/tasks.rs`, with tests on both sides.
- EventKit from Node goes through a tiny Swift helper. **It must be an .app launched with
  `open -W -g -n -a "Helper.app" --args in.json out.json`**: macOS grants Reminders to the
  *responsible app*; a bare CLI run by the agent under launchd blocks forever on a permission
  prompt nobody sees. `LSUIElement` in its Info.plist (no Dock icon); not `LSBackgroundOnly`
  (that suppresses the prompt itself). The person grants access once.
- Configure it in every profile that needs it and disable the agent's old way to the data:
  ```yaml
  # ~/.hermes/profiles/<profile>/config.yaml
  mcp_servers:
    reminders:
      command: /Users/<you>/.local/bin/node      # an absolute path: launchd has no PATH
      args: [/Users/<you>/Projects/reminders-mcp/src/index.js]
  skills:
    disabled: [apple-reminders]                  # Hermes' own AppleScript skill: slow, competes
  ```
  The agent then sees the tools as `mcp__reminders__list_tasks` etc. A gateway restart loads
  them — ask the person to restart when the agent is idle (`hermes gateway restart` refuses
  while work runs).

## Where a Hermes profile keeps what you'll touch

`~/.hermes/profiles/<profile>/`: `config.yaml` (models, `mcp_servers`, `skills.disabled`),
`SOUL.md` (the agent's standing instructions), `memories/` (`MEMORY.md`, `USER.md`), `skills/`
(each skill a folder with `SKILL.md`, grouped by category folders like `productivity/`),
`cron/jobs.json` (scheduled jobs), `cron/output/<job-id>/*.md` (each run's output — read the
last ones to see what a job really produces). The default profile is `~/.hermes/` itself
(`~/.hermes/skills/`, `~/.hermes/config.yaml`).

## Moving data in, and everything that used the old place

Moving a person's records (Things → Reminders: 1318 tasks) is a one-off import, then the old
place is left alone until they're happy. The import is the easy half. Then find **everything
that read the old place** and move it too, or it breaks silently weeks later:
- the agent's skills (Bench: five report/intake skills rewritten for the new tools);
- the agent's memory and SOUL notes that name the old app;
- **scheduled jobs** (`cron/jobs.json`: `{"jobs": [{id, name, prompt, skills, schedule:
  {expr}, enabled, enabled_toolsets, deliver, origin, last_status, next_run_at, …}]}`): rewrite
  each `prompt` for the new tools; in `enabled_toolsets` **replace the old server's name with
  the new one** (`["things","crm"]` → `["reminders","crm"]` — a list is an
  allowlist; enabled MCP servers are added only when the list names none of them); leave a
  `null` as it is (`null` = the cron platform's tools, which include every enabled MCP server);
  fix any date convention the old tool needed (Things needed "to the 29th" for "through the
  28th"; the new `to` is inclusive). Edit the JSON with a script that loads and dumps it (never
  by hand), keep a copy of the old file;
- back up first (`tar -czf ~/.hermes/backups/<what>-<date>.tgz …`).
Things → Reminders mapping that worked (1318 tasks): title → title (a trailing "- 2 часа"
moved to `Spent:`); notes → notes, then the `Tags:` / `When:` / `Spent:` lines; Today/a start
date → the day; This Evening → `When: evening` + today; Someday → `When: someday`, no day;
deadline → the "important" flag (the person used deadlines only for that); completed → completed
with its original date; the Things link `things:///show?id=<uuid>` → the reminder's URL (to find
the source later). Projects, areas, headings and checklists were not used by this person — ask
before dropping them for someone who uses them. No alarms. Check the import by counts (open /
done / with time / flagged) on both sides, a few spot checks by title, and in the Bench itself.

Bench's case, for scale: the intake profile (tasks from Telegram) had three jobs — a weekly
client report (Mondays) and two monthly ones (the 28th: a digest and an hours report) — plus five
skills and the SOUL/memory notes naming Things; the executor profile had no jobs. Find them all: `grep -rli <old-app> ~/.hermes/profiles/*/{cron/jobs.json,skills,SOUL.md,memories}`.

Check each job by running its prompt once from the terminal, not delivered anywhere. Give the
test run **only the read-side toolsets** (`-t reminders`, plus a read-only one the job needs
for links, never a messaging/delivery one) — "don't send" in a prompt is not a guarantee
(`-t` takes MCP server names):
`hermes -p <profile> chat -Q -t <toolsets> -s <skill> --query-file prompt.txt` with "TEST RUN,
don't send anything" on top — and `hermes -p <profile> cron doctor`.

## Groups of chats without a project

Hermes groups sessions only by the folder they run in (its projects are folder-bound; a session
has no tag field). Bench's "Name a group" field in a new session makes `~/Research/<name>`
(locally, or on the SSH host the gateway runs on) and starts the session there; the sidebar
then groups by that folder (layers icon for folders under `~/Research/`), and its "+" starts the
next one there. Bench `connection::group_dir`, `src/ui/chat.rs` `render_project_choice`.
