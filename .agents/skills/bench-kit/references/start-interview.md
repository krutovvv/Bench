# The interview: from "what I do" to the threads they need

Ask in the person's language, a few questions at a time, in plain words (they may not be
programmers). Don't propose anything until you have the picture.

## 1. Questions

**The work**
- What is your job? What does a normal day look like, from start to end?
- What do you produce (code, texts, posts, reports, deals…)? For whom?

**What you work on** — the things that would become projects (a client, a site, a codebase, a
kind of work), and in them the tools a thread attaches:
- What do you keep coming back to all day: a list of tasks, servers, sites you sign in to,
  folders on a host, clients, orders, tickets, documents?
- For each: where does it live now (an app, a website, a server, a file)? What exactly do you do
  with it (the 2–3 actions you repeat)? What do you copy from it, and where to?

**The agent**
- What do you ask an AI for most often? What would you like it to do on its own, or on a
  schedule, or from your phone (Telegram)?
- Which steps between your tools feel mechanical (look up, paste, convert, check, send)?

**The small things**
- Which small sites or utilities do you open for one quick thing (converters, generators,
  validators)? These become tools in Tools.

## 2. Write it back

A short list in their words: "1. Every morning you check the server logs over SSH for errors.
2. You publish posts on three sites by hand. 3. …" Ask them to correct it. This list is the
specification; build exactly this.

## 3. Turn the answers into threads

Bench is one chat with the agent; each thing they work on becomes a **kind of thread**: a group
in the sidebar, a live block at the thread's top, the agent's tool for it
(`build-subject.md`). First see whether one of the five that exist covers it:

| They work with… | Thread | What the block shows | What the agent does |
|---|---|---|---|
| their tasks | Tasks (`tasks:<list>`) | the list: Today, Upcoming, Anytime, Logbook | adds, moves, completes, sums the time spent |
| servers | Servers (`ssh:<alias>`) | the server's terminal | runs commands there (`ssh_run`) |
| files on hosts (FTP, SFTP) | Files (`files:<host id>`) | the folder and the open file | lists, reads, writes, renames (`remote_files`) |
| websites they sign in to | Browser (`browser:<id>`) | each step's page, live; they click and type in it | drives the page through the browser MCP |
| small utilities | Tools (`tool:<name>`) | the tool's input and output | explains, fixes, works with what's in it |
| a project, a question | an ordinary thread (in a folder) | — | reads, edits, runs, searches |

What none of these covers becomes **a new kind of thread**, in the same shape: say what its
sidebar lists, what its block shows, which tool the agent gets (or which MCP server already
exists for that service), what Bench sends with each message. Examples: "my clients" (a CRM's
records, the agent through the CRM's MCP), "the shop's orders" (a list re-read from the shop's
API), "the build queue" (a list of runs with their logs).

## When the job lives in a website or a cloud service

In order, say which one each job gets:
1. **The agent does it through an MCP server or an API** (most such jobs): the thread shows
   the result; no page needed.
2. **The browser thread**, when there is no API or the person needs to see and touch the page
   (sign-ins, publishing on sites without an API): the agent drives the server's browser, the
   person watches each step live and steps in.
3. **A tool in Tools** for the local step around the service (HTML → Markdown of a copied page,
   clean up pasted text).
Never a half-copy of the website inside Bench.

## 4. Example: how the user's Bench came out of their answers

- "I talk to an agent all day, in projects" → ordinary threads, in folders, with git branches.
- "I keep my tasks in a task app and log the hours" → **Tasks**: the lists as threads, the
  agent's `tasks` tool; "how many hours this month for the client" is one question.
- "Many servers, always in a terminal" → **Servers**: each `~/.ssh/config` host a thread with its
  terminal; the agent runs commands in it and reads what's on screen.
- "Sites on old hosting, FTP passwords in Transmit" → **Files**: hosts imported once, a folder and
  an editor in the block, the agent edits through `remote_files`.
- "We publish on sites without an API, a whole team" → **Browser**: a shared server browser,
  each person with their own key, each step's page live in the answer.
- "DevUtils for Base64, regex, cron; tables from sites for the AI" → **Tools**, one thread per tool.
