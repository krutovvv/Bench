// Generated from crates/protocol (cargo test -p protocol). Do not edit: change the Rust types.

export type ThreadItem = { "kind": "user", key: string, seq: number, text: string, at?: number, 
/**
 * The Pi submission that brought it, when the agent's records say.
 */
sub?: string, 
/**
 * Bench's id for the message (the `request_id` it was sent with): the row shown on
 * Enter is this one (docs/thread-objects.md).
 */
request?: string, } | { "kind": "thought", key: string, seq: number, text: string, at?: number, } | { "kind": "text", key: string, seq: number, text: string, 
/**
 * The words of an assistant message that also called a tool: the agent's narrative
 * of what it is about to do, not an answer. Pi's own signal — the message's blocks.
 * `None` is an agent too old to mark them; such texts are judged by their place.
 */
narration?: boolean, at?: number, error?: string, } | { "kind": "tool", key: string, seq: number, call_id: string, name: string, args: unknown, 
/**
 * What it acts on, in one line (a command, a path).
 */
preview: string, at?: number, } | { "kind": "result", key: string, seq: number, call_id: string, name: string, output: string, failed: boolean, 
/**
 * An edit's unified patch.
 */
diff?: string, at?: number, } | { "kind": "insert", key: string, seq: number, id: string, insert_key: string, } | { "kind": "fail", key: string, seq: number, text: string, } | { "kind": "plan", key: string, seq: number, id: string, todos: Array<PlanStep>, } | { "kind": "ask", key: string, seq: number, id: string, 
/**
 * "clarify" or "approve".
 */
ask_kind: string, question: string, 
/**
 * Extra text under the question: the command an approval wants to run.
 */
detail?: string, choices: Array<string>, multi: boolean, } | { "kind": "asked", key: string, seq: number, id: string, answer?: string, };

export type PlanStep = { id: string, content: string, 
/**
 * "pending", "in_progress", "completed", "cancelled".
 */
status: string, parent?: string, };

export type QueuedItem = { key: string, sub: string, mode: string, text: string, 
/**
 * Bench's id for the message, when Bench sent it.
 */
request?: string, };

export type ThreadOpen = { items: Array<ThreadItem>, queue: Array<QueuedItem>, inserts: { [key in string]?: InsertState }, running: boolean, 
/**
 * The live run's holds (the person's pauses), oldest first; the last one open while it
 * stands held. Empty when no run lives.
 */
holds: Array<Hold>, 
/**
 * The thread's background processes, each placed at the call that started it.
 */
processes: Array<Process>, 
/**
 * What the live run waits for the person to answer.
 */
asks: Array<Ask>, info: ThreadInfo, 
/**
 * Its numbers so far; `None` before its first answer.
 */
usage?: Usage, 
/**
 * How fast the model wrote its last answer, as Bench measured and the agent kept it.
 */
speed: SpeedStat, head: number, };

export type InsertState = { 
/**
 * What it is: "ssh:<machine>", "tasks", "tools", "browser".
 */
key: string, 
/**
 * Its own state (the list, the tool, the browser's tab).
 */
state: Record<string, unknown>, folded: boolean, removed: boolean, 
/**
 * After how many of the person's messages it sits, when not where its entry was written.
 */
after?: number, };

export type InsertPatch = { key?: string, state?: Record<string, unknown>, folded?: boolean, removed?: boolean, after?: number, };

export type Ask = { id: string, kind: AskKind, 
/**
 * What the person may answer (a question's choices; an approval's once/session/always/deny).
 */
choices: Array<string>, 
/**
 * An approval's command and why the agent wants it.
 */
command: string, description: string, };

export type AskKind = "clarify" | "approve";

export type ThreadInfo = { 
/**
 * Its reasoning effort; "" is the provider's default.
 */
effort: string, 
/**
 * The fallback model answered its last call because the main one failed.
 */
fallback?: Fallback, };

export type Fallback = { model: string, reason: string, };

export type Usage = { model: string, context_used: number, context_max: number, compressions: number, cache_hit_pct?: number, input: number, output: number, calls: number, 
/**
 * The agent's own average: output tokens over whole-call time (prefill included).
 */
avg_tps?: number, };

export type SpeedStat = { 
/**
 * Tokens per second, one sample for each second of the last answer.
 */
samples: Array<number>, prefill?: number, generation?: number, 
/**
 * Characters per token, as measured.
 */
ratio?: number, };

export type Hold = { from: number, to?: number, };

export type Process = { 
/**
 * The Pi task's id: what Stop names.
 */
id: string, call_id: string, command: string, state: ProcessState, exit_code?: number, 
/**
 * ms, like an item's `at`.
 */
started?: number, ended?: number, 
/**
 * The last lines of its output.
 */
tail: string, 
/**
 * Why it could not run, when it failed.
 */
error?: string, };

export type ProcessState = "running" | "exited" | "stopped" | "restarted" | "failed";

export type StreamKind = "text" | "thought" | "narration";

export type ThreadEvent = { "kind": "append", "value": Array<ThreadItem> } | { "kind": "stream", "value": [StreamKind, string] } | { "kind": "queue", "value": Array<QueuedItem> } | { "kind": "inserts", "value": { [key in string]?: InsertState } } | { "kind": "run", "value": boolean } | { "kind": "holds", "value": Array<Hold> } | { "kind": "process", "value": Process } | { "kind": "asks", "value": Array<Ask> } | { "kind": "info", "value": ThreadInfo } | { "kind": "usage", "value": Usage };

export type Event = { "type": "threads_changed" } | { "type": "thread", thread: ThreadId, event: ThreadEvent, } | { "type": "tasks_changed" };

export type ThreadId = string;

export type Inserts = Record<string, InsertState>;

export type Done = Record<string, never>;

export type ThreadSummary = { id: ThreadId, title: string, messages: number, running: boolean, 
/**
 * The person paused its run.
 */
paused: boolean, 
/**
 * Where it was started: "local" (Bench), "telegram", "cron".
 */
source: string, 
/**
 * The project it belongs to; `None` for the agent's own (Host) threads.
 */
project?: string, 
/**
 * Unix ms of its last change.
 */
updated: number, 
/**
 * The folder it works in, and the top of the git repo that folder is in, if any.
 */
cwd: string, repo?: string, 
/**
 * Who it is with, for a thread from Telegram (the person's name there).
 */
with?: string, };

export type Created = { thread: ThreadId, };

export type CommandResult = { "kind": "output", "text": string } | { "kind": "send", "text": string };

export type Command = { 
/**
 * "/model".
 */
name: string, description: string, };

export type ListThreads = Record<string, never>;

export type CreateThread = { project: string | null, cwd: string | null, };

export type OpenThread = { thread: ThreadId, since: number, };

export type RenameThread = { thread: ThreadId, title: string, };

export type DeleteThread = { thread: ThreadId, };

export type ForkThread = { thread: ThreadId, at: number, };

export type SendMessage = { thread: ThreadId, text: string, context: string, request: string | null, queue: boolean, 
/**
 * The reasoning effort to answer with, when the person changed it.
 */
effort: string | null, };

export type StopThread = { thread: ThreadId, };

export type PauseThread = { thread: ThreadId, paused: boolean, };

export type WithdrawQueued = { thread: ThreadId, sub: string, };

export type SendQueuedNow = { thread: ThreadId, sub: string, };

export type EditQueued = { thread: ThreadId, sub: string, text: string, };

export type PutInsert = { thread: ThreadId, id: string, key: string, state: Record<string, unknown>, };

export type PatchInsert = { thread: ThreadId, id: string, patch: InsertPatch, };

export type AnswerAsk = { thread: ThreadId, ask: string, answer: string, };

export type SetEffort = { thread: ThreadId, effort: string, };

export type SaveSpeed = { thread: ThreadId, speed: SpeedStat, };

export type RunCommand = { thread: ThreadId, command: string, };

export type ListCommands = Record<string, never>;

export type StopProcess = { thread: ThreadId, process: string, };

export type Methods = { "threads.list": [ListThreads, Array<ThreadSummary>], "thread.create": [CreateThread, Created], "thread.open": [OpenThread, ThreadOpen], "thread.rename": [RenameThread, Done], "thread.delete": [DeleteThread, Done], "thread.fork": [ForkThread, Created], "thread.send": [SendMessage, Done], "thread.stop": [StopThread, Done], "thread.pause": [PauseThread, Done], "thread.queue.withdraw": [WithdrawQueued, Done], "thread.queue.send": [SendQueuedNow, Done], "thread.queue.edit": [EditQueued, Done], "thread.insert": [PutInsert, Done], "thread.insert.patch": [PatchInsert, Done], "thread.answer": [AnswerAsk, Done], "thread.effort": [SetEffort, Done], "thread.speed": [SaveSpeed, Done], "thread.command": [RunCommand, CommandResult], "commands.list": [ListCommands, Array<Command>], "process.stop": [StopProcess, Done] };
