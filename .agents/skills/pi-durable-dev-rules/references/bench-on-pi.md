# How the agent uses Pi Durable

Each piece of Bench in Pi's terms. Read the row of what you change, then Pi's own docs beside this
page or its sources (`agent/node_modules/@earendil-works/`).

Pi is used unmodified, as its authors describe it. Our code stays in Pi's terms:

| Piece | In Pi's terms |
|---|---|
| Thread | a Pi conversation in `~/.bench/pi.sqlite`; every step durable, a turn survives a restart. Bench shows its journal: `thread.open` (items after a head, the queue, the inserts' state) and the `thread.*` events from Pi's commits (`agent/src/runtime/journal.ts`) |
| Insert | an entry `bench.insert {id, key}` written where it was put (a write: the model doesn't see it, never queued) + its state in the conversation document `bench.inserts` `{items:{id:{key,state,folded,removed,after?}}}`, changed in commits. `after` moves one whose entry's place is not where it belongs: an insert put while a message waits in the composer goes after that message. RPC `thread.insert`, `thread.insert.patch`, told as `ThreadEvent::Inserts` (`src/inserts/SPEC.md`) |
| What the agent knows of inserts | the `inserts` prompt section, read with `input.read.snapshot(Inserts, conversationId)`, one line per kind (`prompt.ts` `inserted()`), plus `<block-state>` with each message |
| Prompt | Pi sections (soul, environment, inserts, project, skills, notes); all but inserts held as first read until the next compaction (`turn/frozen.ts`): a changed section rewrites the prompt's head and the model server's cache with it |
| Queue | Pi's inbox: a message sent while a run is live is a follow-up; "send now" aborts it and steers. Bench's id for each message is Pi's `requestId`: the row shown on Enter is matched by it, never by its words |
| Plan, questions | the agent's own `todo` and `clarify` calls in the journal, the answer the call's result (`docs/thread-objects.md`) |
| Streams of the model's server | no such limit between conversations in Pi, so ours: the main provider's `parallel` (the gear's "Streams"), a gate in `beforeRequest` (`runtime/streams.ts`). Whatever asks the model takes one: a conversation at work keeps its stream to the end of its run and gives it up while it waits for something else; a title, a review, a compaction outside a run takes one for its request; the others wait in line, in order |
| Pause | the conversation document `bench.hold` (`hold.ts`) and a gate in `beforeTool`: a held run stays held across a restart |
| Code mode | Pi's `codemode` extension (`@earendil-works/pi-codemode`, a QuickJS sandbox): one step runs a script that calls the thread's tools and returns only what it prints. Every thread has it beside the tools it calls directly (`forAgent` adds it to the conversation's extensions at every turn), with no setting: not a mode for code, several steps in one. Scripts reach MCP servers (each a namespace) and `models.*`: `models.classify()` on `bench/evaluator` where the project uses the evaluator |
| Restarts | the agent stopping closes the harness (`runtime.shutdown`), which Pi does not count. A task cut off by crashes 5 times with nothing new is parked: the thread says so and Run again is `harness.retryTask` (`runtime/parked.ts`). A storage failure closes the harness, and the agent ends for its service to bring it back |
| Fork | `conversation.fork(at)`: "Fork from here" on an answer makes a new thread through that entry |
| `/terminal` | tool `ssh_run` (`builtin/ssh.ts`); without a host it takes the thread's terminal insert |
| `/tools` | no tool of its own: the utility's input and output come in `<block-state>` |
| `/browser` | the Playwright MCP through `@earendil-works/pi-mcp` (`mcp__browser__*`), one tab per thread (state `{tab,url}`). The server browser is its own project, `~/Projects/browser-server-mcp` (github.com/krutovvv/browser-server-mcp) (an MCP server with a live view); Bench is one of its clients (`src/inserts/browser/gateway.rs`), watching the gateway the project's `browser` MCP server names as `live`, with the key `~/.bench/ui/browser.json` keeps for it under `keys` |
| Model | one for every thread of every project, set in the gear: the general agent's `defaultProvider`/`defaultModel`; a project has no model of its own. The fallback (`fallback_providers[0]`) is a pi-ai provider of ours (`fallback.ts`): the main model's error or silence sends the same request to it (Pi retries only with the same model), and the thread's `info {fallback}` shows a red chip by its repo |
| Where tools work | Pi's `ExecutionEnv`, one per conversation (`runtime/tools-host.ts` `envFor`): `NodeExecutionEnv` on the agent's machine, `SshExecutionEnv` (`machine/ssh.ts`) on a project's machine over SSH, wrapped by `machine/confined.ts` for a project kept in its folder. Pi's read/write/edit/bash and our grep/glob/ls work through it unchanged |
| Compaction | Pi's compaction policy; a project's share of the window becomes a per-model override under its own provider (`project_<id>.<provider>`), through a getter Pi reads at every use (`compactionOverrides`); 20% of the context stays word for word, and after the summary the evaluator puts the person's words in force, the files in work and exact results (`judge/summary.ts`) |
| After an edit | a `ToolTask` `afterTool` hook (`builtin/diagnostics.ts`) adds the file's diagnostics from the project's language servers (`lsp/lsp.ts`) to edit and write results |
| Read and edit | Pi's `read` wrapped to show every line as `LINE:HASH|text`, and an `edit` of the same name that changes lines by those anchors (`builtin/hashline.ts`): no old text to reproduce, a stale anchor refused with the lines as they are now, the changed lines returned with new anchors. An edit with `oldText`/`newText` is Pi's own edit |
| Code navigation | tool `lsp` (`builtin/navigate.ts`) asks the same language servers: a place is a file, a line and the word on it, never a column. The servers start at a thread's first tool call (`envFor` → `warmUp`); one still loading the project (rust-analyzer's `experimental/serverStatus`) is waited for 20 s, then said to be loading |
| Evaluator | Pi's classifier model `bench/evaluator` (`judge/models.ts`): the gear's System One servers, a main one and its fallback, which our hooks ask at fixed points of a turn and code mode scripts too. A project uses it when its Settings say so (on in a new project). What it does at each point, and the classic way without it: `agent/src/judge/SPEC.md` |
| Step limit | a project's most steps in one turn: refuses calls in `beforeTool` and tells the model to answer in `beforeRequest` (`turn/steps.ts`) |
| Schedule | a durable `defineTask` per job that sleeps until its next run (`builtin/cron.ts`) |
| Background processes | `process` (`builtin/process.ts`): a background task of the conversation (`bench.process`, `background`, `abandonOnRestart`) that runs the command in its `ExecutionEnv`, its output's tail in the checkpoint, its exit the outcome; Bench reads the task records from Pi's commits (`thread.process`). A restart of the agent ends it for good ("stopped by restart") |
| MCP | `@earendil-works/pi-mcp`, a project's `mcp.json`, with Pi's `exposure`: `codemode` by default (`agent/src/mcp/SPEC.md`). OAuth via `mcp/oauth.ts` (callback on `localhost:47823`) |
| Telegram | grammY, one bot per project, the same Pi conversations; `TELEGRAM_PROXY` or `HTTPS_PROXY` in the agent's env |

Extend with Pi's packages and popular ones before writing your own. When something in Pi is
missing or buggy, work around it in our code and say so in a comment. Never patch Pi.
