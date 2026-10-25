<p align="center">
  <img src="assets/logo.svg" width="160" alt="Bench">
</p>

<h1 align="center">Bench</h1>

<p align="center"><b>A client for an AI agent, shaped like Zed's Delta: the agent's work shows up live in the thread — a web page you can click, a terminal, a task list.</b></p>

<p align="center">
  <a href="https://krutovvv.github.io/bench/"><b>Try the demo in your browser</b></a> ·
  <a href="https://github.com/krutovvv/bench/releases">Download for macOS, Linux, Windows</a>
</p>

![A thread with the browser inserted: the thread's own tab, live, where it was put](docs/thread-browser.png)

<sub>A made-up session (demo mode): nothing in these screenshots is real data. The
[browser demo](https://krutovvv.github.io/bench/) is the same app compiled to WebAssembly —
click through the threads in the sidebar.</sub>

## The idea

Zed's [Delta](https://zed.dev/blog/introducing-deltadb) treats the conversation with the agent as
the unit of work. Bench takes that shape — one chat app, no modes, everything a thread — and
shows it best in the **browser thread**.

A thread is a **document**: your messages, the agent's answers, and what you **insert** into it
with `/`. An insert is a small live app, part of what you say, right where you put it: the
thread never switches or changes its name for it.

You type `/browser` and ask the agent to do something on a website: "open the blog's admin, make
a draft from the release notes, stop before publishing". The agent works in a real browser on a
server, in **this thread's own tab**, and the insert shows that tab **live**, with no browser
chrome around it — instead of a screenshot. You watch it work, click and type in the page
yourself (fix a word, tick a box, sign in), then say "continue". Another thread works on another
site in its own tab.

The same goes for my other everyday tools. The sidebar holds only **projects** — a client, a
site, a codebase, a kind of work — and their threads. In any thread, `/terminal`, `/ssh`, `/ftp`,
`/tasks`, `/tools`, `/browser` put the tool in, live; several fit in one thread. The agent knows
what is inserted, sees what each insert shows with every message, and has a tool to act on it.
Coming back to a thread brings its inserts back as they were.

A project carries its own environment, as in Claude Code: a soul (what it is, how to work in it),
its MCP servers, its skills, its tasks, its server and FTP accesses, its scheduled jobs. A folder
on disk is optional. There are no agent profiles to pick: you open the project and work.

| Insert | What's live | What the agent does with it |
|---|---|---|
| **`/browser`** — the thread's tab in a shared server browser | the page: click and type in it | drives it (Playwright), only in this thread's tab |
| **`/tasks`** — Today, Upcoming, Anytime, Logbook a click apart | the list (in a project: its tasks) | adds, moves, completes, sums the hours ("how much did I work for this client in October?") |
| **`/terminal`, `/ssh`** — this Mac, or a `~/.ssh/config` host | the terminal, as tall as its text | runs commands there and reads what's on screen |
| **`/ftp`** — FTP, FTPS and SFTP hosts | the hosts, the folder, an editor for the open file | lists, reads, edits, renames |
| **`/tools`** — Base64, regex, cron, JSON, Markdown from a web page… | the tool's input and output | explains, fixes, works with what's in it |

The project's own settings are in its environment (the pencil): its soul, MCP servers, skills,
prompts, accesses, Telegram bot and **schedule** — the jobs that run a prompt in a new thread of
the project on their own. The gear holds only the providers and the look. Ordinary threads work
in a project folder, with a git branch of their own when needed.

<table>
<tr>
<td><img src="docs/thread-server.png" alt="A server's terminal inserted into a thread"></td>
<td><img src="docs/thread-tasks.png" alt="The task lists inserted into a thread"></td>
</tr>
<tr>
<td><img src="docs/thread-tool.png" alt="The cron parser inserted into a thread"></td>
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

Inserts are kept the Pi way: each is an entry in the conversation's transcript, where it was put
(the model doesn't see it), and its state — which host and folder, which list, which tab — is a
document of the conversation, changed in commits. Bench changes them through the agent and takes
the agent's answer as the truth. An insert kind is three small pieces on the agent's side:
- a **tool** that acts on it (`tasks`, `ssh_run`, `remote_files`, or an MCP server);
- a line of **prompt** saying what it is, built from the inserts document;
- the **insert's state** that Bench sends with each message (the terminal's last lines, the open
  file, the tool's input and output). It goes to the model and is kept out of the history.

A message sent while the agent works waits in the conversation's queue (Pi's inbox) and goes
when the agent answers; above the composer each one can be sent now or taken back.

The browser inserts use a shared Chrome on a server, a tab per thread, which the agent drives
through the Playwright MCP. People watch and step in through a small gateway. All three come as one Docker
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
for **adding such a tool to Bench**: one more kind of insert, made the same way as the five
above, in the same look:

- **the recipe** ([`build-insert.md`](.agents/skills/bench-kit/references/build-insert.md)):
  the insert's key and state, its `InsertKind` (title, switcher, controls, body, what it tells
  the agent), its `/` word, the agent's tool and prompt line, with the browser insert as the main
  worked example and the other four after it, and a checklist of every file a new one touches;
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
| **Inserts**: the model, the `/` list, one `InsertKind` per kind | `app/inserts.rs`, `ui/inserts/` (`terminal.rs`, `files.rs`, `tasks.rs`, `tools.rs`, `browser.rs`), `app/subjects.rs` · agent: `inserts.ts` |
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
BENCH_DEMO=1 ./target/release/bench                # the made-up session, on the browser insert (also: tasks, server, tool, env, landing)
scripts/build-web.sh                               # the browser demo, into web/dist
```

GPUI runs on macOS, Linux and Windows, and in the browser; GitHub builds all of them on every
push (`.github/workflows/build.yml`). I use Bench on macOS. Notifications and the one-time
import of tasks from Apple Reminders talk to macOS directly; the rest is the same Rust
everywhere.

## License

[MIT](LICENSE): use it, change it, grow your own. Nothing here is for sale; it's shared as an
experiment.
