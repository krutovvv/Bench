---
id: env
type: submodule-design
status: draft
title: src/env — a project's environment, section by section
parent: app
depends-on: [client, ui]
references: [explorer, projects-and-agents]
---

# A project's environment

What a project's agent works with — its soul (`SYSTEM.md`), MCP and language servers, skills,
prompts, the model's settings (reasoning, approvals, code mode, compaction), its Telegram bot,
its schedule, where it lives — as Bench edits it. The files themselves are the agent's
(`~/.bench/projects/<id>/` on the project's agent, and a folder's `.agents/skills`); the agent
changes the same files when asked, from any thread of the project.

## The one split

Every part of the environment is a `Section` (its data in `ProjectEnv`, save, revert, reload) and
exactly one of two:

- **`SettingsForm`** (registered in `FORMS`): fields on the project's Settings page (`page.rs`,
  the tab `DocKey::Settings`) — what the project *is set to*: name, where it lives, how it runs,
  Telegram, backup, removal.
- **`ExplorerParts`** (registered in `PARTS`): parts in the Explorer's tree, each opened as a
  document in the center — what the project *contains* that is not a file: an MCP server with its
  state and sign-in, a language server, a scheduled job. Its `group()` is its group in the tree.

What is a file (the soul, prompts, skills) is neither: the Explorer opens it as a file
(`explorer`). A new kind of thing in a project is therefore a file (nothing new), or one
section file here plus a line in `FORMS` or `PARTS` and its part of the agent's
`/api/projects/<id>/env`.

## Invariants

- Each project's environment is its own (`app.envs`, by project key). A view reads the
  environment of the project it shows, never "the current" one.
- One entity, one save path: each MCP or language server is its own entry with its own state,
  never a second editor saving the same JSON file.
- The agent says when an environment changed (`EnvChanged`); Bench does not poll it.

## Boundary

Talks to the agent only through `client` (REST under `/api/projects/…`); never touches the
agent's files directly, so a server's project works exactly as a local one. Export and import of
a whole environment to one file is `backup.rs` here and `agent/src/modules/backup.ts` there.

## Known limitations

Sections take `&mut BenchApp`, and a card's fields are code, not a schema of data
(`architecture` §6).
