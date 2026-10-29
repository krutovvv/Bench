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
results the evaluator changed, and notes beside them.

It takes two switches. The gear's (Settings → Agents → Evaluator, the `judge` setting) is the
connection: the server, its model, its key. A project's (Settings → Model → Evaluator, its
`evaluator` in settings.json, Host's too) is its use, **off unless turned on**: on a shared GPU the
evaluator takes the room the other utilities need — speech to text, image generation, OCR, a
translator — and the balancer stops one to start the other, so it is on in the projects where its
speed is worth it (a lot of code, large volumes) and off where those utilities are used.
`judgeFor(conversation)` is the one place that answers whether a thread uses it; every point,
the excerpt and the window's cut ask it. Off in either, everything goes the classic way.

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
- **plan** (`plan.ts`, `result`, every 5 steps while the plan has open items, reading the agent's
  own last words and its last results at more length than a line): each open item —
  not started, in progress, finished. A sure answer (0.85 finished, 0.7 begun) moves it forward,
  never back; when one finishes and none is under way, the next begins. The thread shows the plan
  so marked (`bench.plan`, read by the journal as a later snapshot of the same plan) and the model
  is told in a line, so it need not spend a step on `todo`. Its own `todo` call wins and the marks
  start again from it. With the evaluator off or failing, the plan's reminder (`planNudge`) does
  this job; with it on, the reminder is silent.
- **course** (`drift.ts`, `result`, every 10 steps): on track, drifting to something not asked,
  looping, or done. Anything but on track, when sure (0.7), is a note with that step's result.
- **done** (`done.ts`, `answer`, after five steps or more): an answer judged unfinished,
  unchecked or off target (0.8) sends the agent back once, with the reason, by Pi's continuation —
  an `<evaluator>` message the thread does not show and that is not a new turn.

- **start** (`start.ts`, `request`, a turn's first): which of the thread's skills fits the request
  (a choice, "none" first) and which MCP servers the work needs (a choice of two each); a sure
  answer is a line with that request: load this skill, use these servers' tools.
- **risk** (`risk.ts`, `call`, approvals "manual" only): a command the patterns let through is
  judged — reads, changes the project, or harm hard to undo; harm (0.5) asks the person, as a
  pattern's hit does. It only adds questions.
- **failure** (`failure.ts`, `result`, a failed call): retry once, fix the call, another way, or
  ask the person (0.6), said with the error.
- **excerpt** (`excerpt.ts`, called by `builtin/window.ts`): a result past its share of the window
  is cut into parts of 1 500 characters, the parts put in the state (9 000 characters a request),
  and a choice over their names (and "none", last) keeps the parts that stand out; without an
  answer, the cut keeps head and tail as before.

- **repeat** (`repeat.ts`, `call`, when the same tool was called in the last 3 steps): the call is
  new, repeats a failed one, or repeats one whose result the agent has, nothing changed since; a
  repeat (both kinds summed, 0.85) is refused once with the reason — the same call again runs.
- **listing** (`listing.ts`, `result`): a JSON list of 8 items or more (a shop's or a web search's)
  comes ordered by use for the request, the items that stand out first, every item kept.
- **claims** (`claims.ts`, `answer`): an answer with numbers after 3 steps or more is checked
  against all the turn found (`turn.found`, 40 results); unsupported (0.8) sends it back once.
- **keep** (`keep.ts`, called by `builtin/window.ts`): when a request would overflow the window, the
  earlier results the next step needs least are cut first.
- **start** also offers the named servers' tools from the turn's next step (`control.addTools` on
  its first result, as `tool_search` does).

What the local server (h2o-lightning-4b behind its shim, `MAX_LEN` 16384) taught: what a question
is about goes in the state, not in its instructions or options — a part's text in a yes/no's
instructions read every part alike (0.20), in an option it was read from its start; in the state,
yes/no and choice both find it (0.94). A record longer than the server's context is shortened per
question by the shim (BM25, the parts nearest the question kept), so a request carries at most
about 9 000 characters of parts. "none" first takes nearly all the probability, so where finding
matters it comes last.

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
