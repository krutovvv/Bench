<p align="center">
  <img src="assets/logo.svg" width="160" alt="Bench">
</p>

<h1 align="center">Bench</h1>

<p align="center"><b>A desktop client for coding with an AI agent on your own models.</b><br>
Local and LAN models over an API, projects on this machine or on your servers, chat and an IDE in one window.</p>

<p align="center">
  <a href="https://github.com/krutovvv/bench/releases/latest"><b>Download</b></a> (macOS, Linux, Windows) ·
  <a href="https://krutovvv.github.io/bench/"><b>Try the demo in your browser</b></a>
</p>

<p align="center">
  <img src="docs/shots/tour.gif" alt="A short tour: a new project, a chat with the agent, its folded steps, the review of a turn, the Code view, a diff, a server's terminal, a project's settings">
</p>

> **Alpha.** Everything works and I use it every day, but it is not yet as fast or as reliable
> as I want it to be, especially with remote projects. Expect rough edges and changes.

## What Bench is, in four lines

1. **Just a client.** Bench talks to models over an API: first of all local ones, on your Mac or a
   machine in your network (any OpenAI- or Anthropic-compatible API), or a provider you have a key for.
   There are no subscriptions, no accounts and nothing of ours in between.
2. **Made for code.** You can use it for deep research or writing, but everything in it is built
   around working on code: vibe coding, reviewing what the agent did, fixing it by hand.
3. **Work is organized in projects.** A project lives on this machine or on a remote one; for you
   it is the same.
4. **The agent can live anywhere.** On your Mac, or on a server on its own, so Bench becomes a
   control center for several machines.

All screenshots below are a made-up demo session: nothing in them is real data.

## Projects

The project is the smallest unit in Bench. The sidebar holds projects, and each project holds its
threads. A project says **where its work happens**:

- **Local**: a folder on this Mac; the local agent works in it.
- **A server, from here**: a folder on a server you reach over SSH. The agent on your Mac works
  there; nothing is installed on the server.
- **A server's own agent**: the agent is installed on the server and runs there by itself. Your
  Mac is not needed: close the laptop and the work goes on.
- **No folder**: an assistant for research, writing, Telegram.

A project can **keep the agent in its folder**: its tools refuse paths outside it, its commands
cannot write outside it. Or you can give the agent the whole machine.

Each project has its own environment, and nothing of it leaks into another project: its soul
(`SYSTEM.md`), prompts, skills, MCP servers, language servers, a schedule of jobs and a Telegram
bot.

![A new project: local, a server over SSH, a server's own agent, or no folder](docs/shots/new-project.png)

## Chat and Code

Every project has two views, switched in the title bar.

**Chat** is an ordinary chat with the agent. The agent's work does not flood it: all its actions
fold into one line ("8 actions · 3:17 · Done"), and while it works you see only the text of what it
is doing right now. Unfold the line to see every step.

<table>
<tr>
<td><img src="docs/shots/chat.png" alt="A chat: the agent's actions folded into one line, then its answer with a chart"></td>
<td><img src="docs/shots/actions.png" alt="The same turn with its actions unfolded: reads, edits, a dev server, web fetches"></td>
</tr>
</table>

**Code** is the same project as a built-in IDE: the file tree, documents in tabs, language servers,
lines changed since the last commit, and a chat beside the editor. Look at what the agent did in
the chat, select lines and send them to it, point it at an exact place in an exact file, or just
fix the code yourself and save.

![The Code view: the project's files, an open document, the thread beside it with the selected lines attached](docs/shots/code.png)

Because a project can live on a server, this works the same for a remote folder: open a server's
project (even production, if you dare) and work with it, with the agent or by hand, as if it were
on your computer. No separate editor, no copying files back and forth. It saves a lot of time and
lets you keep many more projects going at once.

## Reviewing what the agent did

- **Changed files.** Under each answer, the files that turn changed, with their line counts. A
  click shows a file's diff in place; one more opens it in the Code view.
- **A built-in reviewer.** "Review" at the head of the block has an independent reviewer read the
  turn (what you asked, the plan, what the agent says it did, and the diff) and leave notes on the
  exact lines. "Send to the agent" hands them back as review comments.

<table>
<tr>
<td><img src="docs/shots/review.png" alt="Changed files under an answer and the reviewer's two notes"></td>
<td><img src="docs/shots/diff.png" alt="A file's changes since HEAD in the Code view"></td>
</tr>
</table>

## The agent's settings, per project

- **Code mode or classic tool calls.** In code mode the model writes one script that calls many
  tools in a single step: fewer steps for work in code. Or the standard agent loop, a tool call per
  step.
- **An evaluator.** A small, fast model (a System One server, Jev's API) decides where the main
  model spends its steps: it refuses a repeat of a call that already failed, trims long results to
  what the request needs, checks the course on long runs. Some steps get much faster.
- **Compaction.** When a long conversation is summarized, as a share of the model's window.
- **A step limit** for long sessions: past it the agent stops and answers with what is done.
- **Reasoning and approvals**: how hard it thinks, and what it may run without asking.
- **A fallback model** that takes over when the main one fails or goes silent.

![A project's settings: code mode, the evaluator, the step limit, compaction, the Telegram bot](docs/shots/settings.png)

## Telegram

Give a project a Telegram bot and its chats become that project's threads. You can talk to your
project from your phone: ask what the agent is doing, give it a task, read the answer.

## Many machines, one window

Add a server in Settings → Agents and Bench installs the agent there over SSH, starts it and keeps
it at the same version as itself. Each server's agent has its own projects, models and Telegram
bot, and runs on its own. Bench becomes a control center: you add as many machines as you need and
check on them, watch them and update them when you want.

## Commands in the chat

Type `/` in a thread and a small live tool is put right where the conversation is. The agent sees
what it shows with every message and can act on it.

| Command | What you see | What the agent does with it |
|---|---|---|
| **`/terminal`** | a terminal on the thread's machine (this Mac or a server) | runs commands there and reads the screen |
| **`/browser`** | the thread's own tab in a browser on a server; you can click in it | drives the tab while you watch and step in |
| **`/tools …`** | small utilities: Base64, JSON ↔ YAML, cron, regex… | sees the input and the output, explains, fixes |

![A server's terminal in a thread: the agent found what makes the site slow](docs/shots/server.png)

Messages you send while the agent works wait in a queue. A run survives a restart of the app,
because every step is kept on disk.

## What it is built on

- **[Pi Durable](https://github.com/earendil-works/pi)** runs the agent: a small Node server in
  [`agent/`](agent/README.md), with Pi used unmodified and extended with its own packages. Every
  step of a conversation is durable.
- **[GPUI](https://www.gpui.rs)**, the UI framework of the [Zed](https://github.com/zed-industries/zed)
  editor, through gpui-kit. Bench is written from scratch in Rust.
- **The look is macOS's**: Finder's sidebar, Messages' bubbles, System Settings' forms.
- **The structure and logic** come from Claude Code's desktop app and from Zed's workspace (its
  docks, panels and tabs).

Bench is also built from inside itself: I talk to the agent in Bench, and it changes Bench's code,
checks the change on its own copy of the app and rebuilds it. [`AGENTS.md`](AGENTS.md) is what an
agent reads first.

## Build

```bash
cargo build --release && ./target/release/bench    # run it
BENCH_DEMO=1 ./target/release/bench                # the made-up session (also: landing, server, explorer, env)
scripts/build-web.sh                               # the browser demo, into web/dist
cd agent && npm ci && node --test test/*.test.ts   # the agent's tests
```

The agent needs Node.js 22.18 or newer; Bench installs and starts it by itself. GitHub builds
macOS, Linux, Windows and the browser demo on every push, and a release on every tag.

## License

[MIT](LICENSE).
