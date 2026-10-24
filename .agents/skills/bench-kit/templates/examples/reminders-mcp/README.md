# reminders-mcp

MCP server (Node.js) for Apple Reminders as a task list, for Hermes agents. Tasks live in
iCloud Reminders (the Reminders app, iPhone, Calendar); Bench shows them like Things.

- `src/index.js` — the MCP tools: `list_tasks`, `get_task`, `list_tags`, `add_task`,
  `update_task`, `complete_tasks`, `delete_task`.
- `src/notes.js` — what Reminders has no field for, kept as lines at the end of the notes
  (`Tags: a, b` · `When: evening|someday` · `Spent: 1h 30m`), the same lines Bench reads.
  "Important" = Reminders' high priority; a task's day = the reminder's date (no time, no alarm).
- `helper/` — "Reminders Bridge.app": the EventKit half in Swift, started through `open` so macOS
  grants Reminders access to it once, whoever runs the MCP (the Hermes gateway runs under
  launchd → osascript, which cannot answer a permission prompt itself).

```bash
npm install
npm run build:helper      # bin/Reminders Bridge.app (swiftc + ad-hoc codesign)
npm test                  # notes format, durations, days
node test/e2e.mjs         # through a real MCP client; creates and deletes one test task
```

Hermes profile (`config.yaml`):

```yaml
mcp_servers:
  reminders:
    command: /Users/<you>/.local/bin/node
    args:
      - /Users/<you>/Projects/reminders-mcp/src/index.js
```
