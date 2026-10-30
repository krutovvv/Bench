# Where a tool's data lives

A tool's records must be reachable by the agent with Bench closed — in Telegram, on a schedule,
from another machine — and readable by Bench. So they never live only in Bench's memory or in
Bench's view state. Two places, in this order:

1. **Where they already live**, when that place is the person's long-lived one: servers in
   `~/.ssh/config`, files in their folders, sign-ins in the browser's profile,
   a service's records behind its API or MCP server. Bench shows them there; the agent reaches
   them with its own tool or MCP server.
2. **`~/.bench/bench.sqlite`**, Bench's one home, owned by the agent: projects, cron jobs, providers, settings (among them `agents`, the
   servers' agents, `servers.ts`; `presets`, the person's own, `presets.ts`). Bench reads and
   writes it **through the agent's REST API** (`/api/projects`, `/api/agents`;
   routes in `agent/src/modules/<area>.ts`, e.g. `projects.ts`), never by opening the file; the
   agent through its tool (`agent/src/builtin/projects.ts`).
   One store, two clients.
3. **`~/.bench/projects/<id>/`**, a project's environment as files (an agent directory in Pi's
   terms): `SYSTEM.md`, `mcp.json`, `lsp.json`, `skills/`, `prompts/`, and `files/`, where a
   project without a folder works (`projects.dir`, `agent/src/builtin/projects.ts`).

Not: a JSON or SQLite file of Bench's own, a third-party app as the store, a database inside
Bench.

## Who else writes there

The agent (its tool), cron jobs, Telegram, another Bench window, another machine. So Bench
re-reads the list now and then and never assumes it is the only writer (`build-insert.md`, "a
list re-read from its source": keep unsaved new items, don't overwrite a card being typed into).

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

## Projects without a folder

The sidebar holds only projects. A project's folder (a codebase) is optional: without one its
threads work in `~/.bench/projects/<id>/files` (`projects.dir`). An "audit" or a "research" of a
few deep threads is such a project.
