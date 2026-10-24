<p align="center">
  <img src="assets/logo.svg" width="160" alt="Bench">
</p>

<h1 align="center">Bench</h1>

<p align="center"><b>A personal workbench, and the kit an AI agent uses to build one for anyone.</b></p>

![A Bench at work: the agent's steps, then its answer](docs/chat-steps.png)

<sub>A made-up session (demo mode, `BENCH_DEMO=top`): nothing in these screenshots is real data.</sub>

## The idea

Professional tools are built for everyone. Each of us uses a small, specific part of a dozen of
them — the part our own work needs — and pays for the rest with clutter, with switching
windows, with copying things from one app into the next.

Nobody makes software for one person: nobody else knows their process well enough, and it would
never pay off. An AI agent can, if it knows *how* to build it well. **A Bench is one window
holding exactly the parts of your work you use, built by an agent from your own description.**
Everyone's tools are different, so everyone's Bench is different. They all have the same shape.

## How it started

I'm a developer, and my day was spread over a dozen apps: an AI chat (none of the clients I
tried fit — [BoltAI](https://boltai.com), DeepSeek Harness, Hermes' own UI, Hermes One,
[TypingMind](https://www.typingmind.com), [Open WebUI](https://openwebui.com)), SSH servers in
[Warp](https://www.warp.dev) (with ideas from Zap, its open-source fork), small files in yet
another editor, tasks in [Things](https://culturedcode.com/things/), and
[DevUtils](https://devutils.com) for Base64, regex, JSON and the like. Each of them is good.
Together they meant switching windows all day.

The push came from [ArtCraft](https://thedieline.com/artcraft-launches-an-open-source-ai-powered-alternative-to-adobes-creative-cloud/):
a developer used Claude to write open-source Rust look-alikes of Adobe's apps — PhotoCraft after
Photoshop, FilmCraft after Premiere ([Phoronix](https://www.phoronix.com/news/Photocraft-Filmcraft-AI)).
If an AI gets that far with programs of that size, the much simpler programs one person uses
every day can certainly be brought together into one.

So I tried. **Today I use one app instead of fifteen, and I spend much less time getting
things done.** Nothing was cloned: the goal was to take the idea of each piece I actually use
and give it one shape. Mostly we worked from screenshots and descriptions; here and there we
looked deeper with [REA](https://github.com/morluto/rea) (Reverse Engineer Anything), an MCP for
taking apart binaries and applications, to see how a program does something before rebuilding
just that part. The same route works for any program, and any workflow, you'd like to fold in.

The interesting part turned out to be not my app but the way to make one: written down as
agent skills, it works for anyone's set of tools.

## The kit is the point

[`.agents/skills/bench-kit`](.agents/skills/bench-kit/SKILL.md) is a set of agent skills. It
doesn't describe my tools; it describes *how to make anyone's*:

- **how to interview a person** about their work and turn the answers into the shape of an app;
- **one look for every Bench**, written down to the pixel: the frame, a catalog of every piece
  (buttons, rows, bars, forms, panes, cards) with pictures and code, the rules behind them;
- **how to grow it**: add a mode, a tool, a card, a native data source, a connection to an agent;
- **how to change it safely**: a fixed loop of measuring, editing, building and measuring again,
  with scripts that check the result to the pixel, so a modest model can do it without guessing;
- proven libraries, ready recipes, and the mistakes already paid for.

Think of a 3D printer and its test boat, the "benchy": one model, printed in different sizes and
colors for different purposes, always recognisably the same. The kit is the printer.

## What every Bench keeps

1. **One look, and the look is the method.** Flat, square, precise: a sidebar, a header, a status
   bar, exact spacing, one accent color. A job placed into "a list on the left, a strip of small
   controls, a few panes, a status line" splits into the right pieces by itself. The agent
   doesn't design, it assembles, which is why even a mid-level model can build one well.
2. **Native and light.** Rust with [GPUI](https://www.gpui.rs) (the UI framework behind the Zed
   editor) through [gpui-kit](https://crates.io/crates/gpui-kit): GPU-drawn, no Electron, no
   bundled browser. This whole Bench — chat, terminals, editor, tools, tasks, every icon, font
   and highlighting grammar — is one 75 MB binary that starts instantly.
3. **It aggregates; your data stays where it lives.** In my case tasks stay in the system's
   Reminders, servers in `~/.ssh/config`, files in their folders. A Bench keeps no database of its
   own, and everything is still there if you open the native app instead.
4. **An AI agent, if you want one.** Most workbenches turn around an agent that takes over the
   steps between your tools; the kit has a complete client for [Hermes Agent](https://hermes-agent.nousresearch.com).
   An app without one is just as much a Bench.
5. **You grow it yourself.** When you need something new, you ask the agent again, and it is
   added the same way, in the same look.

![The same answer, further down: code, a video, a voice note, sources, a running process, changed files](docs/chat-media.png)

## The code here is mine — and the agent's

The app in `src/` is **my** Bench. It fits my work, not yours, and you don't need its pieces.
It's here because an agent building *your* Bench works better with a real, finished one at hand:
every page of the kit points into this code. Take what's useful, delete the rest. The pieces:

| Piece | What it is | Files (in `src/`) |
|---|---|---|
| **Frame** | window, sidebar, header, status bar, theme, icons, shortcuts | `main.rs`, `theme.rs`, `icons.rs`, `ui/mod.rs`, `app/keys.rs`, `bench.rs` |
| **AI agent client** (the most reusable part) | chat with Hermes: sessions, steps, subagents, media, voice, files it changed, background processes, profiles, providers, prompts, skills, cron, notifications | `gateway.rs`, `rest.rs`, `connection.rs`, `history.rs`, `media.rs`, `audio.rs`, `video.rs`, `attach.rs`, `notify.rs`, `app.rs`, `app/{asks,attachments,cron,gateways,install,marks,processes,profiles,projects,prompts,providers,slash}.rs`, `ui/{chat,composer,media,asks,sidebar,statusbar,panels,providers,install}.rs` |
| SSH and terminals | servers from `~/.ssh/config`, a terminal per server | `ssh.rs`, `sshconfig.rs`, `term.rs`, `remote.rs`, `app/servers.rs`, `ui/servers.rs`, `ui/terminal.rs` |
| Mini IDE | file tree, editor, tabs, diffs | `app/{ide,ide_ops,ide_tabs}.rs`, `ui/files.rs`, `diff.rs`, `lang.rs` |
| Tools | everyday utilities (Base64, regex, JSON, cron…) as tested pure functions | `devtools/`, `app/tools.rs`, `ui/tools.rs` |
| Tasks | a task list over the system's Reminders | `tasks.rs`, `reminders.rs`, `app/tasks.rs`, `ui/tasks.rs` |
| Demo | the made-up session in these screenshots | `demo.rs` (its media: `assets/demo/`) |

SSH, the mini IDE, Tools and Tasks can each be switched off with one flag in
[`src/bench.rs`](src/bench.rs);
the kit's [`bench-modes-remove.md`](.agents/skills/bench-kit/references/bench-modes-remove.md)
explains what goes with each.

## A Bench that builds itself

This Bench is developed from inside itself: I talk to the agent in Bench, the agent changes
Bench's code, and every commit becomes its own app next to the one I'm running. I try the new
one; when it's good I switch to it; the previous builds stay for rolling back.

## Build

```bash
cargo build --release && ./target/release/bench    # run it
BENCH_DEMO=1 ./target/release/bench                # the made-up session from the screenshots
```

GPUI runs on macOS, Linux and Windows (and has a web backend). This Bench is built and used on
macOS, and a few of its pieces talk to macOS directly (the Reminders tasks, notifications,
`scripts/bundle-macos.sh` for an `.app`); on another system those are the parts to replace or
switch off — the rest is plain Rust.

## Use the kit

Open this folder as the working folder of an agent session (in Bench, or in Hermes — trust it
once with `hermes skills trust <path-to-this-repo>`), describe your work, and ask for your
Bench. The agent starts from [`SKILL.md`](.agents/skills/bench-kit/SKILL.md).

---

<sub>About the design: before this I built [iSite.app](https://isite.app), a platform where web
studios make websites and online stores for their clients without programmers — its visual
editor and interface included. Much of how Bench looks and is put together comes from that
work.</sub>
