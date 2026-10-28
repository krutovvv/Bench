---
id: agent-judge
type: submodule-design
status: draft
title: agent/src/judge — the evaluator, and the step limit beside it
parent: agent
references: [thread-objects]
---

# The evaluator

A System One model (TypeSafe's Jev, or a server with its API: `POST /v1/systemone`, a state and
typed questions in, each option's calibrated probability out, in well under a second) that the
agent asks at fixed points of a turn, so that decisions the thread's model would spend a step on,
or never make, are made fast. The thread's model never calls it and never sees it: it sees
results the evaluator changed, and notes beside them. It is the agent's, not a project's: one
switch in the gear (Settings → Agents → Evaluator, the `judge` setting), because on a local GPU
it shares the card with speech to text and the person chooses one or the other.

## Where it is asked

Every point is a Pi hook in the default extension (`extensions/assistant.ts`), on what
`builtin/turn.ts` keeps of the turn: the person's request, the plan, the steps in a line each.

- **A search** (`search.ts`, `afterTool` on grep, glob, a grep or find through bash): a list of
  twelve lines or more comes with the likeliest places for the request first, with their
  probability; the others follow in their order. Nothing is dropped.
- **The course** (`drift.ts`, `afterTool`, every 20 steps): on track, drifting to something not
  asked, looping, or done. Anything but on track, when sure (0.7), is a note with that step's
  result, as the plan's reminder is.
- **The end** (`done.ts`, `onYield`): after five steps or more, an answer judged unfinished,
  unchecked or off target (0.8) sends the agent back once, with the reason, by Pi's
  continuation — an `<evaluator>` message the thread does not show and that is not a new turn.

The safe side wins every tie: the option that changes nothing is listed first (the model leans to
the first), and below its threshold an answer changes nothing.

## Off, unset or failing

Every point does nothing and the turn goes the classic way. A failure (refused, timed out after
60 s — a local server loads its model on the first request — or an HTTP error) keeps that address
quiet for a minute. Every request and its answer is a line in `~/.bench/judge.log`, to measure the
evaluator by; the thread's meters count its requests and how often it changed the course
(`Usage.judge_calls`, `judge_nudges`).

## The step limit (`builtin/steps.ts`)

Not the evaluator's, but on the same turn: a project's most steps in one turn (Settings → Model →
Step limit, none by default). Past it every call is refused with the reason and every request to
the model carries the instruction to answer with what is done and what is left; a model that
keeps calling tools is stopped six refusals later. A turn stopped by the limit is not sent back
by the evaluator.

## Known limitations

- What a turn is, is kept in memory: after a restart of the agent a turn's steps count from zero.
- The evaluator's state is cut to 24 000 characters (a local server's window is about 16K tokens).
