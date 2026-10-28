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

## Points and moments

Every place the evaluator is asked at is a `Point` (`point.ts`): the moment of a turn it may ask
at, whether it asks now and what (typed questions, and what it adds to the state), and what the
answers change — a result reordered, a note to the model, the turn sent back, an entry for the
thread, the plan's marks. `index.ts` keeps them in `POINTS` and asks every point of a moment that
has a question now in one request: one state (the turn's request, plan and last steps, kept by
`builtin/turn.ts`, plus what each point adds), every question evaluated against it in parallel,
each point's answers under its own names. The default extension's hooks call the moments
(`extensions/assistant.ts`): `result` in `afterTool`, `answer` in `onYield`.

- **search** (`search.ts`, `result`: grep, glob, a grep or find through bash): a list of twelve
  lines or more comes with the likeliest places for the request first, with their probability; the
  others follow in their order. Nothing is dropped.
- **plan** (`plan.ts`, `result`, every 5 steps while the plan has open items): each open item —
  not started, in progress, finished. A sure answer (0.85 finished, 0.7 begun) moves it forward,
  never back; when one finishes and none is under way, the next begins. The thread shows the plan
  so marked (`bench.plan`, read by the journal as a later snapshot of the same plan) and the model
  is told in a line, so it need not spend a step on `todo`. Its own `todo` call wins and the marks
  start again from it. With the evaluator off or failing, the plan's reminder (`planNudge`) does
  this job; with it on, the reminder is silent.
- **course** (`drift.ts`, `result`, every 20 steps): on track, drifting to something not asked,
  looping, or done. Anything but on track, when sure (0.7), is a note with that step's result.
- **done** (`done.ts`, `answer`, after five steps or more): an answer judged unfinished,
  unchecked or off target (0.8) sends the agent back once, with the reason, by Pi's continuation —
  an `<evaluator>` message the thread does not show and that is not a new turn.

The safe side wins every tie: the option that changes nothing is listed first (the model leans to
the first), and below its threshold an answer changes nothing.

## Growing it

A new idea is a file with a `Point` and a line in `POINTS`; a new moment is a dispatcher in
`index.ts` and the hook that calls it. The likeliest next ones, each a narrow decision the thread's
model now spends a step or a long context on:

- **before a request** (`beforeRequest`): which skills and tools a turn needs (instead of the
  model's `tool_search`, and the 170 tools it once loaded); whether the request is a question to
  answer at once or work to plan; whether to ask the person first (`clarify`).
- **after a result**: what of a long file, page or command output matters for the request (the
  model reads the likeliest parts first, as with a search); a failed command — retry, fix, or ask;
  deep research's sources and passages scored before the model reads them.
- **before a call** (`beforeTool`): how dangerous a command is, for approvals (instead of the
  regular expressions); whether a call repeats one that just failed.
- **at compaction**: which entries to keep word for word.
- **memory**: whether an exchange is worth remembering.

## Off, unset or failing

Every point does nothing and the turn goes the classic way. A failure (refused, timed out after
150 s — a local server behind a GPU balancer starts and loads its model on the first request, 90 s
measured — or an HTTP error) keeps that address quiet for a minute. Every request and its answer is a line in `~/.bench/judge.log`, to measure the
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
