<p align="center">
  <img src="assets/logo.svg" width="160" alt="Bench">
</p>

<h1 align="center">Bench</h1>

<p align="center"><b>A client for an AI agent, shaped like Zed's Delta: the agent's work shows up live in the thread — a web page you can click, a terminal, a task list.</b></p>

<p align="center">
  <a href="https://krutovvv.github.io/bench/"><b>Try the demo in your browser</b></a> ·
  <a href="https://github.com/krutovvv/bench/releases">Download for macOS, Linux, Windows</a>
</p>

![A browser thread: the agent's step is the live page, right in its answer](docs/thread-browser.png)

<sub>A made-up session (demo mode): nothing in these screenshots is real data. The
[browser demo](https://krutovvv.github.io/bench/) is the same app compiled to WebAssembly —
click through the threads in the sidebar.</sub>

## The idea

Zed's [Delta](https://zed.dev/blog/introducing-deltadb) treats the conversation with the agent as
the unit of work. Bench takes that shape — one chat app, no modes, everything a thread — and
shows it best in the **browser thread**.

You ask the agent to do something on a website: "open the blog's admin, make a draft from the
release notes, stop before publishing". The agent works in a real browser on a server. Each of
its steps shows up **as the page itself, live, inside its answer**, with no browser chrome
around it. You see what it did, and you can click and type in that page yourself: fix a word,
tick a box, sign in. Then ask for the next step. Earlier steps keep the last picture of their
page, so the thread reads like a record of the work.

The same idea covers my other everyday tools: **each one is a kind of thread**. The sidebar holds
only **projects** — a client, a site, a codebase, a kind of work — and their threads. In a thread
you attach a tool with `/` (`/ssh web-1`, `/ftp`, `/browser`, `/tasks`, `/tool`): it stays live at
the top, the conversation underneath. The agent knows what the thread is about, sees what the
block shows with every message, and has a tool to act on it.

A project carries its own environment, as in Claude Code: a soul (what it is, how to work in it),
its MCP servers, its skills, its tasks, its server and FTP accesses, its scheduled jobs. A folder
on disk is optional. There are no agent profiles to pick: you open the project and work.

| Thread | What's live | What the agent does there |
|---|---|---|
| **Browser** — sessions in a shared server browser | each step's page, in the answer: click and type in it | drives the page (Playwright), one tab per turn |
| **Tasks** — Today, Upcoming, Anytime, Logbook | the task list | adds, moves, completes, sums the hours ("how much did I work for this client in October?") |
| **Servers** — every `~/.ssh/config` host, and this Mac | the server's terminal | runs commands there and reads what's on screen |
| **Files** — FTP, FTPS and SFTP hosts | the folder and an editor for the open file | lists, reads, edits, renames |
| **Tools** — Base64, regex, cron, JSON, Markdown from a web page… | the tool's input and output | explains, fixes, works with what's in it |

Ordinary threads work in a project folder, with a git branch of their own when needed. Scheduled
jobs and Telegram chats are threads too.

<table>
<tr>
<td><img src="docs/thread-server.png" alt="A thread about a server: its terminal live at the top"></td>
<td><img src="docs/thread-tasks.png" alt="A thread about today's tasks"></td>
</tr>
<tr>
<td><img src="docs/thread-tool.png" alt="A thread about the cron parser"></td>
<td><img src="docs/chat-media.png" alt="An answer with code, a video, a voice note, sources, a running process, changed files"></td>
</tr>
</table>

## The agent

Bench is a client. The agent is [`agent/`](agent/README.md), a small Node server on
[Pi Durable](https://github.com/earendil-works/pi). It is used the way its authors describe it
and extended with their packages and popular ones; about 3k lines are our own. Pi keeps every
step on disk, so a turn survives a crash, and cron jobs and Telegram answers keep working with
Bench closed. Bench installs and starts the agent on this Mac (`~/.bench`), or over SSH on a
server; it needs Node.js 22.18 or newer.

A thread about something is three small pieces on the agent's side:
- a **tool** that acts on the subject (`tasks`, `ssh_run`, `remote_files`, or an MCP server);
- two or three sentences of **prompt** saying what "this" and "here" mean in that thread;
- the **block's state** that Bench sends with each message (the terminal's last lines, the open
  file, the tool's input and output). It goes to the model and is kept out of the history.

The browser threads use a shared Chrome on a server, which the agent drives through the
Playwright MCP. People watch and step in through a small gateway. All three come as one Docker
setup in [`browser-server/`](browser-server/README.md), ready to deploy. Each person gets their own
key, no SSH needed. It allows a narrow set of operations and keeps a log. Frames come at 2 per
second at most, and only while someone is watching. Idle tabs sleep to keep the server within
its memory budget.

## How it got here

My day used to be spread over a dozen apps: an AI chat client (none fit quite right), SSH
servers in [Warp](https://www.warp.dev), FTP in Transmit, tasks in
[Things](https://culturedcode.com/things/), [DevUtils](https://devutils.com) for Base64 and
regex, a browser for the sites we publish on by hand. Each is good. Together they meant
switching windows all day.

The first plan was bigger than this: a collection of skills from which an agent would build
*anyone* a different app from a description of their work. ArtCraft's
[rebuilds in pure Rust](https://github.com/storytold) of whole heavy programs made that seem
possible. Along the way it got simpler. The modes, the separate panels and the "build any app"
went away, one at a time. What stayed is a Delta-like agent client where each tool is a thread.
It wasn't a straight road, but that's where it landed, and it's what I now work in all day.

## The kit: add your own tools as threads

Your tools are not mine. One person has clients, another the shop's orders, a third a build
queue. [`.agents/skills/bench-kit`](.agents/skills/bench-kit/SKILL.md) is a set of agent skills
for **adding such a tool to Bench**: one more kind of thread, made the same way as the five
above, in the same look:

- **the recipe** ([`build-subject.md`](.agents/skills/bench-kit/references/build-subject.md)):
  the subject's key, the live block, its `/` command, the agent's tool, its prompt section
  and the block's state, with the browser thread as the main worked example and the other four after it and a checklist of
  every file a new one touches;
- **how to ask** a person about their work and turn the answers into threads;
- **one look, written down to the pixel**: a catalog of every piece with pictures and code, the
  rules behind them, and what I liked and rejected along the way;
- **how to change it safely**: a fixed loop of measuring, editing, building and measuring again,
  with scripts that check the result, so even a modest model can do it without guessing;
- proven libraries, the agent's side on Pi, and the mistakes already paid for.

Open this folder in an agent session (in Bench, or another agent that reads skills), describe
what you work on, and ask for it. For someone else's Bench, `scripts/new-bench.sh` makes a copy
to grow.

## The code

The app in `src/` is my Bench. The kit points into it everywhere, because a real, finished app
is the best example. Take what's useful.

| Piece | Files (in `src/` unless noted) |
|---|---|
| **Frame**: window, sidebar, header, status bar, theme, icons, shortcuts | `main.rs`, `theme.rs`, `icons.rs`, `ui/mod.rs`, `ui/sidebar.rs`, `ui/statusbar.rs`, `app/keys.rs`, `bench.rs` |
| **Agent client**: threads, steps, subagents, media, voice, changed files, background processes, profiles, providers, prompts, skills, cron | `gateway.rs`, `rest.rs`, `connection.rs`, `pi_agent.rs`, `history.rs`, `media.rs`, `audio.rs`, `video.rs`, `app.rs`, `app/*.rs`, `ui/chat.rs`, `ui/composer.rs`, `ui/media.rs`, `ui/asks.rs`, `ui/panels.rs` |
| **Threads about something** | `app/subjects.rs`, the block in `ui/chat.rs` |
| Tasks | `tasks.rs`, `task_store.rs`, `app/tasks.rs`, `ui/tasks.rs` · agent: `builtin/tasks.ts` |
| Servers | `sshconfig.rs`, `ssh.rs`, `term.rs`, `app/servers.rs`, `ui/servers.rs`, `ui/terminal.rs` · agent: `builtin/ssh.ts` |
| Files | `app/remote_files.rs`, `ui/remote_files.rs` · agent: `builtin/remote.ts`, `builtin/hosts.ts` |
| Browser | `remote_browser.rs`, `app/sites.rs`, `ui/sites.rs` · the server: [`browser-server/`](browser-server/README.md) |
| Tools | `devtools/` (19 tools as tested pure functions), `app/tools.rs`, `ui/tools.rs` |
| Mini IDE beside a thread: tree, editor, tabs, diffs | `app/ide*.rs`, `ui/files.rs`, `diff.rs`, `lang.rs` |
| Demo: the made-up session in these screenshots | `demo.rs` (media in `assets/demo/`) |
| The agent | [`agent/`](agent/README.md) |

## Developed from inside itself

I talk to the agent in Bench, and the agent changes Bench's code. Every commit becomes its own
app next to the one I'm running. I try the new one and switch to it when it's good; the
previous builds stay for rolling back.

## Build

```bash
cargo build --release && ./target/release/bench    # run it
BENCH_DEMO=1 ./target/release/bench                # the made-up session, on the browser thread (also: tasks, server, tool, landing)
scripts/build-web.sh                               # the browser demo, into web/dist
```

GPUI runs on macOS, Linux and Windows, and in the browser; GitHub builds all of them on every
push (`.github/workflows/build.yml`). I use Bench on macOS. Notifications and the one-time
import of tasks from Apple Reminders talk to macOS directly; the rest is the same Rust
everywhere.

## License

[MIT](LICENSE): use it, change it, grow your own. Nothing here is for sale; it's shared as an
experiment.
