---
id: agent-tools
type: submodule-design
status: draft
title: agent/src/builtin — the agent's own tools and stores
parent: agent
depends-on: [agent-machine]
references: [thread-objects, projects-and-agents]
---

# The agent's tools

The tools the default "assistant" extension gives a thread beside Pi's own read, write, edit and
bash, and the small stores behind them: `ssh_run` (`ssh.ts`), background processes, cron,
subagents (`delegate`), memory, skills, `clarify` and approvals (`ask.ts`), projects, the
servers' agents, presets, grep/glob/ls (`search.ts`), code navigation (`navigate.ts`, the `lsp`
tool), and line-anchored read and edit (`hashline.ts`). Each tool's line in the prompt is in
`agent/src/prompt.ts`.

## Boundary

- A tool reaches the server only through `host.ts` (which session a conversation is, asking the
  person, running a prompt of its own), and the files and shell only through the conversation's
  `ExecutionEnv` (`agent-machine`). So every tool works the same on this Mac, on a server over
  SSH, and in a project kept in its folder, without knowing which.
- A tool that leaves something in the thread leaves it in Pi's terms (`thread-objects`): the plan
  is the `todo` call's arguments, a question is the `clarify` call and its result, a process is a
  conversation background task (`bench.process`), a subagent is a child conversation owned by the
  tool's task, a scheduled job is a durable `defineTask` sleeping until its next run.

## Decisions

- **Edits by anchors, for weak models.** `read` shows every line as `LINE:HASH|text`; `edit`
  names lines by those anchors instead of reproducing old text — the step weak and fast models
  fail most. A stale anchor is refused with the lines as they are now, never applied elsewhere.
  An edit with `oldText`/`newText` is Pi's own edit, unchanged.
- **Code navigation names a place as a file, a line and the word on it, never a column.** It asks
  the project's language servers, started at a thread's first tool call; one still loading the
  project is waited for 20 s, then said to be loading.
- **A background process does not outlive the agent**: `background` (a run's Stop and end leave
  it alone) and `abandonOnRestart` (a restart ends it for good, "stopped by restart").
- **Memory is per project**: a project's notes are its own (`memories/MEMORY.md` in its
  environment); who the person is (`USER.md`) is the general agent's, shared.
- The list of servers' agents is this Mac's agent's setting (`servers.ts`), not project rows.
