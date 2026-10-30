# Bench's concepts

The exact words for what Bench is made of, used unchanged in code, specs and answers. Read this
before a change to threads, inserts, projects, agents or the Explorer.

Bench is a native desktop **workbench for working with an AI agent**. It is an open,
thread-centric app, written from scratch in Rust and GPUI
(gpui-kit 0.8). It is a client of its own agent in `agent/`, a Node server on **Pi Durable**.

Bench is not a kit for building other apps, and it has nothing to do with Hermes (an earlier
agent, now gone, with nothing of it left on the wire). There are no profiles for the person: there
are projects. A server's agent is always reinstalled to match this Bench (`launch.rs`).

- **Thread**: a document of **messages** (the person's, the agent's, with its steps) and the
  person's **inserts**. One conversation in Pi. It never switches mode or gets renamed because of
  an insert. Bench shows Pi's journal of it as rows (`crates/thread`), on a list that **nothing
  moves but the person** — a run that ends, a row that grows, an answer that arrives never scroll
  it (`docs/thread-rewrite.md`).
- **Insert**: a small live tool put into a thread with `/`. The words are `/terminal`, `/browser`
  (where the project has the server browser's MCP) and each utility's own (`/base64`, `/json-yaml`…;
  `/tools …` lists them all). Every `/` adds a new insert where the conversation is. An insert:
  - shows something live (a terminal, a utility, a browser tab);
  - tells the agent what it shows with every message (`<block-state>`, kept out of the thread Bench
    shows; the model's history keeps it, as taking it out would rewrite the prompt and its cache);
  - has an agent tool that acts on it;
  - is restored exactly on return.

  Kinds that can be live only once (the utilities) show "click to show it here" on the older
  ones.
- **Card**: a form or an editor that appears in the thread as an agent-message block, under the
  insert that asked for it (a task). A project's parts that are not files (an MCP server, a cron
  job) open as documents in the Explorer instead.
- **Agent**: a process of `agent/` that runs threads, its machine's service (launchd on a Mac,
  systemd on Linux: it runs without Bench, comes back when it dies or the machine restarts, and
  one per machine). This Mac's always; a server's when the person adds it in the gear (Bench
  installs it, here or over SSH, the same way: `src/client/service.rs`). Closing Bench stops no
  agent; Restart in the agent's popover restarts its service. The Mac's agent keeps the
  list of servers' agents. Each agent has its own models, voice and general Telegram bot.
- **Project**: the sidebar's only unit. It belongs to one agent and says where its work is
  (`docs/remote-projects.md`):
  - **agent**: this Mac's or a server's;
  - **machine**: the agent's own, or — with this Mac's agent — a server it works on over SSH
    (nothing installed there);
  - **folder** on that machine, optional; its agent can be **kept in it** (tools refuse paths
    outside, commands cannot write outside).

  In code a project is a `ProjectKey` (agent, id there) and where it lives a `Location`;
  `src/app/places.rs` answers "where" for everything else. Its **environment**:
  - soul (`SYSTEM.md`);
  - MCP servers (`mcp.json`, OAuth where needed), language servers (`lsp.json`);
  - skills, prompts;
  - model: reasoning, approvals, evaluator, step limit, compaction;
  - Telegram bot;
  - schedule (cron).

  What it *is set to* (name, where it lives, model, Telegram) is its **Settings** (the last row of its Explorer); what it
  *contains* (its folder, soul, prompts, skills, MCP and language servers, schedule) is the
  **Explorer** (`docs/explorer.md`). The agent changes the same files when asked, from any
  thread of the project. Nothing of one project's environment reaches another, Host's
  included: its soul, MCP servers, skills, prompts, memory and settings are its own threads'
  alone (only the model is the gear's, one for all). **Host** is an agent's own environment: this Mac's is always first in the
  sidebar (installing, building, looking after the machine); a server's (e.g. Dev) names that
  server and is followed by the server's own projects. `/terminal` opens on the thread's machine
  (`thread_machine()`).
- **Explorer**: the third way to look at a project, beside threads and Settings — its files and
  the agent's texts as an IDE shows them (a tree in the sidebar, documents in tabs, the thread
  beside). It is the familiar view of a project, built into Bench: no external editor is
  referenced or needed.
  The window is Zed's workspace: a left dock (threads or Explorer), the center, a right dock.
  A thread sits beside the documents in the Code view; two threads side by side (`src/thread/beside.rs`)
  stay in code but are not offered: two runs at once ask more of a local model than most have.
- **The gear** (Bench → Settings… in the macOS menu, ⌘,) holds the app's settings, in tabs: Agents (each one's models, voice and evaluator servers;
  add and remove servers' agents), Presets (MCP and language servers a project takes with one
  click, compaction shares, reasoning levels), Appearance.
