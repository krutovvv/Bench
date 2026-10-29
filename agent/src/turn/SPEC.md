---
id: agent-turn
type: submodule-design
status: draft
title: agent/src/turn — a turn as the default extension's hooks keep it
parent: agent
references: [agent-judge]
---

# The turn

Not tools: the default extension's hooks (`extensions/assistant.ts`) on every request, call,
result and compaction, and what they keep about a conversation between them.

- `turn.ts` — the turn: the person's request, the plan, the steps and the last of them, what the
  step limit and the evaluator (`agent-judge`) read.
- `steps.ts` — the step limit, a project's most steps in one turn (its section in `agent-judge`).
- `window.ts` — what is sent fits the model's window.
- `frozen.ts` — the prompt's sections held until the next compaction.
- `compaction.ts` — Pi's compaction, its summary made without reasoning.
- `kept.ts` — everything above kept per conversation, in one map.

## Decisions

- **Kept in memory, in one place** (`kept.ts`): each part is a slot of one map by conversation,
  dropped whole when the Runtime closes or deletes the thread. None of it is the thread's record
  (that is Pi's journal): after a restart of the agent a turn's steps count from zero, which errs on
  the side of working.
- **What is sent fits the model's window** (`window.ts`): a tool's result past its share (12%)
  keeps its head and tail with a line on how to ask for less, and the notes the hooks add after it
  (diagnostics, a spec, the evaluator's) follow it uncut; a request that would overflow has its
  largest old results and call arguments cut in that request only, the provider's own count
  correcting the estimate. Pi's compaction still does the lasting work.
- **The prompt's head holds until a compaction** (`frozen.ts`): a section that changed (a note
  written, the date) would rewrite the head of every later request and the model server's prefix
  cache with it; a compaction rewrites what follows the head anyway, so there they are read anew.
- **The summarizer is Pi's, without reasoning** (`compaction.ts`): Pi summarizes with the thread's
  model and reasoning level and has no setting for the summarizer's; its `beforeCompact` hook
  supplies a summary in Pi's own format with reasoning off.
