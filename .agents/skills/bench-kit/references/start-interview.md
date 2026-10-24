# The interview: from "what I do" to a Bench

Ask in the person's language, a few questions at a time, in plain words (they may not be
programmers). Don't propose anything until you have the picture.

## 1. Questions

**The work**
- What is your job? What does a normal working day look like, from start to end?
- What do you produce (code, texts, images, models, reports, deals…)? For whom?

**The programs** — for each program they name:
- Why do you open it? What exactly do you do there (the 2–3 actions you repeat)?
- What do you never use in it?
- What do you copy from it, and where to?

**The routine**
- What do you do many times a day that feels mechanical (convert, rename, look up, paste,
  check, send)?
- Which small sites or utilities do you open for one quick thing (converters, generators,
  calculators, validators)?
- What do you ask an AI for most often? What would you like it to do on its own?

**The data**
- Where do your files live (this Mac, servers, cloud folders, a git repo)?
- Which services do you log into for work (a CRM, a server, a database, an API)?

**The feel**
- Which app's look and feel do you like? (Ours is Zed-like: flat, light, dense — keep it unless
  they object.)
- What annoys you most in the programs you use now?

## 2. Write it back

A short list in their words: "1. Every morning you open the server logs over SSH and search for
errors. 2. You convert tables from the client's site to Markdown for the AI. 3. …" Ask them to
correct it. This list is the specification; build exactly this.

## 3. Turn answers into modes

A **mode** is a top-level place of the window (a button in the header or status bar and a
shortcut, ⌘1 ⌘2 ⌘3…), with its own sidebar and main area. Group the jobs by *what the sidebar
lists*:

| The person works with a list of… | Mode | Sidebar | Main area |
|---|---|---|---|
| conversations with an AI agent | Sessions | chats by project | the chat |
| servers, hosts | SSH | servers by folder | a terminal |
| files of a project | IDE (a panel, not a mode) | the tree | an editor |
| small utilities | Tools | tools by group | input → output |
| records in a service (clients, tickets, posts) | e.g. "CRM" | records, filters | the record, actions |
| media (screenshots, renders, clips) | e.g. "Assets" | folders, recent | a preview, actions |

One-off jobs without a list become **tools** inside the Tools mode. Jobs that link two places
become **actions** ("open in the editor", "send to the agent", "use as input").

The AI agent is the automation center: in Bench it sees the open files, writes to servers through
an MCP server, and the person sends anything to it from any mode ("Add to chat").

## When the job lives in a website or a cloud service

Google Docs, Telegram, a uniqueness checker, stock photo sites, a CRM: the Bench doesn't embed
them (no web views, no headless browser — the user rejected those as heavy). Three honest
options, in this order — say which one each job gets:
1. **The agent does it.** Hermes reaches the service through an MCP server or a skill (Telegram
   intake, Google Drive, an API with a key in the profile's `.env`); the Bench shows the result
   (a chat answer, a task, a file). Most such jobs land here.
2. **A tool for the local step** around the service: HTML → Markdown of a copied page, crop to
   1200×630 for a cover, clean up pasted text — pure functions in the Tools mode.
3. **A link out**: a row action "Open in Google Docs" (`cx.open_url`) — the Bench keeps the
   list, the website keeps the editing.
Never build a half-copy of the website inside the Bench.

## 4. Example: how the user's Bench came out of their answers

- "I talk to a local AI agent all day" → **Sessions** (Hermes client).
- "I keep many servers and I'm always in a terminal" → **SSH** (servers from `~/.ssh/config`,
  terminals, a local one too).
- "I open Zed only to look at a file and fix a line" → the **mini IDE** panel (tree, editor,
  tabs, ⌘P, ⌘D), local and remote files; Zed one click away for more.
- "I use DevUtils for Base64, regex, cron, Unix time; I copy tables from sites for the AI" →
  **Tools** (Rich Text to Markdown first).
- "I want to see what the agent runs in the background" → a card under the answer, not a mode.
