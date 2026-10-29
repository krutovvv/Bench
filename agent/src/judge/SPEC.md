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

It is Pi's classifier model `bench/evaluator` (`models.ts`), one model for two users: these points,
and code mode scripts, which ask it with `models.classify()` for many small decisions over the data
they hold (Pi's design: MCP fetches, the script processes, the classifier decides, the frontier
model reasons). The gear (Settings → Agents → Evaluator, the `judge` setting) lists System One
servers, as many as the person keeps, one the main and one its fallback; the pair answers as one
model: the main server, or the fallback when it fails (a failed server is quiet a minute). There is
no switch there: no server, no evaluator. A project's switch (Settings → Model → Evaluator, its
`evaluator` in settings.json, Host's too) is its use — **on in a new project**, off where its
utilities need the shared GPU (the balancer stops one to start the other). `evaluatorFor(agent)`
is the one place that answers whether a project's threads use it: the prompt's line on it, the
thread's meters and a script's `models.classify()` (refused with the reason in `beforeTool`) ask it;
every point, the excerpt and the window's cut ask `judgeActive(conversation)` — that, and a server
not quiet after a failure. Off, everything
goes the classic way. A script's calls of the classifier are not steps the points weigh or results
they read.

## Points and moments

Every place the evaluator is asked at is a `Point` (`point.ts`): the moment of a turn it may ask
at, whether it asks now and what (typed questions, and what it adds to the state), and what the
answers change — a result reordered, a note to the model, the turn sent back, an entry for the
thread, the plan's marks. `index.ts` keeps them in `POINTS` and asks every point of a moment that
has a question now in one request: one state (the turn's request, plan and last steps, kept by
`turn/turn.ts`, plus what each point adds), every question evaluated against it in parallel,
each point's answers under its own names. What a point's `apply` needs of its asking (the skills
or servers it offered, the likeness it compared) comes back with the answers as the asking's
`context`, never through a map by conversation: Pi runs a step's tools in parallel, and two calls of
one thread are asked about at once. The default extension's hooks call the moments
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
  start again from it. With the evaluator off or failing, the plan's reminder (`turn/turn.ts` `planNudge`) does
  this job; with it on, the reminder is silent.
- **course** (`drift.ts`, `result`, every 5 steps): on track, drifting to something not asked,
  looping, or done. Anything but on track, when sure (0.7), is a note with that step's result.
  Leaning the same way off course (drifting or looping at 0.4) at two checks running is a softer
  note with the person's request: in the backup thread drifting stood at 0.42–0.47 four times over
  twenty minutes and nothing was said. Off course again after either word (sure, or leaning), the
  note says to stop and answer — the soft word alone was read and passed by. After a question, an
  approval or a choice (`intent.ts`) the course is checked every 3 steps.
- **done** (`done.ts`, `answer`, after five steps or more): an answer judged unfinished,
  unchecked or off target (0.8) sends the agent back once, with the reason, by Pi's continuation —
  an `<evaluator>` message the thread does not show and that is not a new turn.

- **intent** (`intent.ts`, `request`, before the first step on each message of the person): what the
  message is, read against the agent's answer it follows (`turn.previous`) — a task, a question,
  an approval or feedback, a correction, or a choice among the agent's options (0.6). The line
  with the request says how far the turn goes: a task all of what it names and nothing else (more
  is offered in a line, not done), a question words, an approval a few lines and no change, a
  correction exactly what it names, a choice what was chosen. The turn keeps the reading, and
  every later question of the evaluator sees it (`baseState`'s `what_the_message_asks`). An agent
  that does everything right does too much: told "fine, not critical" after "the tools are
  final", it spent 36 steps building three things nobody asked for.
- **start** (`start.ts`, `request`, a turn's first): which of the thread's skills fits the request
  (a choice, "none" first) and which MCP servers the work needs (a choice of two each); a sure
  answer (a skill 0.8, a server 0.75) is a line with that request: load this skill, use these
  servers' tools. At the thread's opening message (`turn.opening`; a later one leans on what was
  said before, which the evaluator does not see) it is also asked, with the thread's project and
  folder and the person's other projects in the state, whether the request leaves open what only
  the person can say — which project the work belongs to, which program, what result; sure (0.75),
  the line says to ask with `clarify` before any other step. Sixty steps went to the wrong project
  before the person said "not here, in Assistant TG".
- **step** (`step.ts`, `request`, every request of a turn but its first): does the step about to be
  taken need reasoning — sure it is plain (0.85; never right after a failure), the request goes
  without it (`runtime/steer.ts`: a word for the request's provider session, taken where the
  model's API is registered; reasoning off changes only the template's end, never the prompt's
  head); and do an MCP server's tools, untouched this turn, fit the step (0.85) — a line for that
  step, once a server: the tools offered stay as they were, a changing list would rewrite the
  prompt's head and the server's cache with it.
- **quiet** (`quiet.ts`, `result`, bash, ssh_run, grep, glob that did not fail): does the output show
  a failure all the same — an error printed, a traceback, nothing found where something was
  meant to be (0.8): said with it, before the next step leans on it.
- **watch** (`watch.ts`, `result`, `process log` of one still running): working, finished, failed
  or stuck (0.8), said with the log — "still working" sends the agent to other work instead of
  polling again at once.
- **skill** (`skill.ts`, `result`, every 10 steps): the skill that fits what the agent does now,
  not yet named or loaded this turn (0.8), a note once.
- **boundary** (`boundary.ts`, `result`, every 3 steps from 0.6 of Pi's threshold to where its
  background compaction starts): has a part of the work just ended (0.8) — then the conversation
  is compacted now (`Host.compact`, Pi's `conversation.compact`, in the background), once until a
  compaction brings the context under half the threshold: a summary at a boundary loses less
  than one mid-edit.
- **risk** (`risk.ts`, `call`, approvals "manual" only): a command the patterns let through is
  judged — reads, changes the project, or harm hard to undo; harm (0.5) asks the person, as a
  pattern's hit does. It only adds questions.
- **failure** (`failure.ts`, `result`, a failed call): retry once, fix the call, another way, or
  ask the person (0.6), said with the error. "Another way" still lets the agent find out why (a
  path not where it thought is looked for, not given up).
- **excerpt** (`excerpt.ts`, called by `turn/window.ts`): a result past its share of the window
  is cut into parts of 1 500 characters, the parts put in the state (9 000 characters a request),
  and a choice over their names (and "none", last) keeps the parts that stand out; without an
  answer, the cut keeps head and tail as before.

- **repeat** (`repeat.ts`, `call`): which earlier call of the last 3 the next one is the same
  as is the code's to say, on the whole arguments — a short value (a path, an offset, a pattern)
  that differs is another call; a long one (a command, a script) mostly alike by its words (0.8) is
  a near copy. A line's start cannot say it: in a thread restoring skills from a backup every
  `cd /tmp && python3 - <<'EOF' import json…` read alike in 200 characters, and 12 different calls
  were refused as repeats. The evaluator, shown both calls whole and what was done between, judges
  what the code cannot: new (another part, a fixed error, something changed since), a repeat of a
  failed one, or of one whose result the agent has. A repeat (both kinds summed, 0.85; a near copy
  0.95, as it is often a fix) is refused once with the reason — the same call again runs. A call
  refused before it ran (a repeat, the step limit, the person) is no failure and no earlier result
  (`markNotRun`): the next call is not read as repeating it, and the failure point is not asked.
  A script's nested calls (codemode) are asked about too, and kept whole for it
  (`seeNestedResult`, not steps of the turn): a script is no way around a refusal, or around
  `risk` with approvals on.
- **listing** (`listing.ts`, `result`): a JSON list of 8 items or more (a shop's or a web search's)
  comes ordered by use for the request, the items that stand out first, every item kept.
- **claims** (`claims.ts`, `answer`): an answer with numbers after 3 steps or more is checked
  against all the turn found (`turn.found`, 40 results); unsupported (0.8) sends it back once.
- **keep** (`keep.ts`, called by `turn/window.ts`): when a request would overflow the window, the
  earlier results the next step needs least are cut first. At compaction (`turn/compaction.ts`,
  `keptThrough`) the latest 48 results are weighed for what the rest of the work needs word for
  word — exact text, ids, paths a summary loses — and those (0.3, at most 8 000 characters) follow
  the summary under "Kept word for word". What follows a summary (`summary.ts`, `afterSummary`,
  12% of the window): the person's words still in force (their latest 20 messages, each judged
  past or in force, 0.6) word for word; the files the summed-up part read or changed that the
  work is still on (0.7), as they are now on the thread's machine; then the results, in the room
  left. In a thread compacted 25 times, 38% of the files touched in the 20 steps after a
  compaction had been touched in the 80 before it: read again because the summary lost them.
- **correction** (`correction.ts`, called by `Runtime.submit`): a message the person queues while
  the run works is asked about — a next task, or a correction of the work under way; a sure
  correction (0.8) joins the run after its current tools (Pi's steer), as "send now" would,
  instead of waiting while the run finishes the wrong way.
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

## Who decides stop and done

Each question has one owner, so no two speak about the same thing:

- **Stopping a turn** is the step limit's alone (`turn/steps.ts`, not the evaluator's): past it
  calls are refused and the model is told to answer. Neither course nor done is asked then.
- **A single call that repeats an earlier one** is `repeat.ts`'s, before it runs (the call
  refused once). **A run of steps going round** is `drift.ts`'s `looping`, after a result (a
  note): it judges the course of the last steps, never one call against another.
- **The work leaving the request, or looking done while the agent goes on** is `drift.ts`'s, mid
  turn (a note; it never ends the turn).
- **Whether the final answer is done** is `done.ts`'s, at the answer (sent back once).
- **The plan's marks** are `plan.ts`'s; with the evaluator off or failing, the plan's reminder
  (`turn/turn.ts` `planNudge`, which reads the same plan the turn keeps from `todo`).

## Growing it

A new idea is a file with a `Point` and a line in `POINTS`; a new moment is a dispatcher in
`index.ts` and the hook that calls it. The likeliest next ones, each a narrow decision the thread's
model now spends a step or a long context on:

- **after a result**: what of a long file, page or command output matters for the request (the
  model reads the likeliest parts first, as with a search); deep research's sources and passages
  scored before the model reads them.

Left out on purpose: **memory** (whether an exchange is worth remembering) — the agent writes
memory only when the person asks (`builtin/memory.ts`), and remembering by itself would not make
a turn shorter.

## Off, unset or failing

Every point does nothing and the turn goes the classic way. A failure (refused, timed out after
150 s — a local server behind a GPU balancer starts and loads its model on the first request, 90 s
measured — or an HTTP error) keeps that address quiet for a minute. Every request and its answer is a line in `~/.bench/judge.log`, to measure the
evaluator by; the thread's meters count its requests and how often it changed the course
(`Usage.judge_calls`, `judge_nudges`).

## The step limit (`turn/steps.ts`)

Not the evaluator's, but on the same turn: a project's most steps in one turn (Settings → Model →
Step limit, none by default). Past it every call is refused with the reason and every request to
the model carries the instruction to answer with what is done and what is left; a model that
keeps calling tools is stopped six refusals later. A turn stopped by the limit is not sent back
by the evaluator.

## Known limitations

- What a turn is, is kept in memory: after a restart of the agent a turn's steps count from zero.
- Every state is fitted to 20 000 characters before it is sent (`fitState`: the largest member
  shrinks, a list losing its oldest items, named parts keeping their names); a server that still
  says its window is exceeded (Russian and code are dense in tokens) is asked once more with half.
  At 24 000 and with the turn's results unfitted, the answer's check failed on long turns (2 of 4 in that thread).
