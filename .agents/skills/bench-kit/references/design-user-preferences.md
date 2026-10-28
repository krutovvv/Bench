# What the user wants (from many rounds of feedback)

Dated where it changed. The person's later decisions win over earlier ones: a rule marked
*superseded* is history, kept so it is not brought back.

## Look and feel

- Reference: the Zed editor. Flat, monolithic, dense but breathing. No shadows, no gradients,
  no "cards floating on a background". *Superseded:* "no rounded corners" (rounding was once
  removed as "отвратительный вид"); now every block is `rounded_md` and controls use the theme
  radius 4 / 6, the same everywhere (`crates/ui/src/theme.rs`).
- Light theme by default; dark is optional (Light / Dark / System + accent + S/M/L size).
- Fonts: happy with 14 UI / 12 code; adjust only by small steps, never big redesigns. Chat text
  (answers and own messages) was `text_sm` like session titles ("идеально"), then on
  2026-10-24 one size up, `text_base`, with a bit more inset: the middle is what is read (as in
  Zed). Disliked there: mixed fonts between blocks.
- Syntax: One Light (One Dark in dark mode), the same in every editor and diff.
- Icons: Lucide, monochrome, small. Liked them a lot.
- Spacing is the most frequent complaint. Wants one rhythm ("like a step / golden ratio"):
  equal insets on all sides of a block, facing bars of equal height, rules that line up across
  panels, no 1px "steps" between neighbouring borders. Points out 1–3 pt differences — measure.
- Text in single-line places is truncated with an ellipsis, never wrapped.
- Wrapped titles: tight line height (1.25), not looser than the gap to the line below.
- No scrollbars. Navigation aids are quiet (a rail of ticks).
- Only the thread scrolls: no scroll inside a scroll (the terminal excepted); editors are as tall
  as their text (`editor_height`), lists as long as they are, an empty state is one line.
- Hover-only actions; nothing jumps or changes size on hover/selection (color only).
- Nothing jumps on load either: opening a chat with images/diagrams must not shift the view
  after the first frame.
- One tone per bar: every label and icon in the status bar the same muted color.
- Who speaks is an avatar in a left gutter (2026-10-24; the edge-to-edge question band before
  it is *superseded*); the sidebar's right rule went too ("олдскульно") and the chrome got
  darker. The columns are `CHAT_LEFT` / `CHAT_TEXT` / `CHAT_RIGHT` in `crates/ui/src/metrics.rs`.
- The user's own messages are selectable/copyable like the answers.
- Media in the chat itself — images, video playing in place, audio, Markdown, diagrams,
  formulas; anything else opens in its default app on a click. All with the lightest possible
  implementation (no browsers), see `references/build-media.md`.
- Color labels: thin stripe + light tint of the row background, for quick scanning.
- Working items (a session whose agent runs) show a moving highlight over their title
  (`ShimmerText`, follows the text color) besides the "working…" meta, so they are found at a glance.

- Sidebar projects as a file explorer: icon + name + chevron/"+" on the right, rows on an
  indent guide; no counts, no horizontal rules (`design-layout.md`, Sidebar).
- Tabs and lists that can overflow scroll sideways rather than squeeze; code editors don't
  soft-wrap in narrow panels (horizontal scroll).

- **Every bar the header's height, every crossing rule one line.** The user spots a 1px step
  between neighbouring bars and calls it "бесит". All bars 36 with their rule.
- **No text-size setting** (removed 2026-10-24): one size, tuned; ⌘+ / ⌘− still zoom.
- **Insets in small steps.** "Чуточку больше" means one step (16 → 20 → 24 over two rounds),
  the same number on every side and in every place of the area (message, answer, composer).
  A big app's generous padding (Claude Desktop) is the direction, not the target.
- **No modes** (mode toggles over the sidebar: *superseded* 2026-10-24). The sidebar holds only
  projects and their threads (2026-10-25); tasks, servers, files, tools and the browser are
  inserts put into a thread with `/` (`build-insert.md`). New window is the header's right end;
  the status bar's right holds icons only: background terminals, attach, dictate, the gear, Stop.
- **Cards are frames, not fills**: code blocks, audio, video, Changed files — a 1px rounded
  frame, no background; heads all `block_head` (`BLOCK_HEAD_H` 32). A darker code background read as "тёмный вид".
- **Code in answers has a header**: language left, Copy right, like every AI chat.
- **Fields can read as text**: a name and "Select folder" on one line, the field without a
  frame, sized to its placeholder; one column of stacked controls looked worse.
- **Lists that grow must stay even**: day steps in Upcoming identical however many there are.

## Behaviour

- English UI throughout.
- No dialogs for simple things: defaults + inline editing + arm-to-confirm for deletes.
- Status bar: only items agreed with the user. Pickers there show their icon inside the button;
  readouts open detail popovers on click (stats, speed chart with per-second bars).
- Elapsed time of a running job counts from the job's own start time (from the backend), never
  from when the view was opened: switching chats or restarting the app must not reset it
  (agents run for hours while the user is elsewhere).
- Live numbers that matter most: generation speed (tokens/s, live, per second, with min/avg/max),
  prefill speed, context used/limit (from config), compactions, cache hit. Estimates marked `~`.
- Remembers state: the open thread, the sidebar's width and folds, prefs.
- Things that belong to the backend live there as files the user can edit elsewhere (prompts as
  `prompts/*.md` in the agent dir or the project's environment, skills, SYSTEM.md) — not in
  app-private storage.
- Multi-window rather than tabs: one window = one independent context.
- Git when needed, never forced: the repo and branch are detected from the thread's folder or
  edited files and shown as a label; no git actions of Bench's own (branches per thread were
  dropped). Chats with no files show nothing git-related.
- Dictation (the status bar's mic, ⌘⇧D): the mic stays the mic while it records, a small level
  bar on its edge; the typed text is never hidden (a thought is dictated in pieces between
  typing); a spinner while transcribing. *Superseded:* a waveform over the field and a round
  filled Stop in the composer.
- Nothing slow starts by itself: version checks etc. run from an explicit button.
- Placeholders are misleading: hide model / effort / approvals until the real values arrive.
- Restart / Stop must actually work and say why when they can't.
- Standard shortcuts everywhere (⌘S save, ⌘N new, ⌘⇧N new window, ⌘, settings, Esc cancels
  recording/forms…), with the shortcut in the button's tooltip. Esc must still reach a focused
  terminal (vim) — a root `capture_key_down` takes only ⌘ combos and Esc in specific states.
- One source of truth, no app-private copies of what the system already stores: servers live in
  `~/.ssh/config` (app fields as `# folder:` / `# path:` comments), keys as `IdentityFile`,
  never passwords.
- No personal data or tokens left on servers. "Nothing installed" holds only for this Mac's
  agent working on a server as a project's machine (over SSH). A server's own agent (added in
  the gear) and a project's language servers are installed there by Bench
  (`docs/remote-projects.md`).
- Lightweight first, the heavy tool one click away: the whole project opens in Zed from the
  project's ⋯ menu (only where it has a folder).

- **The Bench aggregates; data stays native** (servers in `~/.ssh/config`). The user rejected a JSON store, then SQLite, then a
  third-party app (Things) as the store, then an MCP built into the Bench: the agents' MCP lives
  in the agent, the Bench only shows and edits (`build-native-data.md`). *Updated 2026-10-24:*
  tasks and sessions now live in `~/.bench/bench.sqlite`, owned by the agent (`agent/`), which
  Bench reads and writes through it; still no store of Bench's own.
- No iPhone notifications from tasks (no alarms); back-dated completion is welcome; tags matter
  more than lists or priorities.
- After moving data, the scheduled reports must keep working: check every cron job and skill
  that touched the old place.
- Projects without a folder are wanted ("an audit", "a research" of 3–5 deep researches): a
  name only, no folder picking (its work goes to a folder Bench keeps for it).

## How to work with the user

- The user describes problems by voice in Russian, often several at once, mid-task: acknowledge each
  briefly, keep going, address all of them, report per item.
- Wants honest status: what was checked, what only builds, what needs the user's click.
- "Don't hack": prefer supported APIs; if something isn't supported (e.g. downgrading the
  backend), say so and offer the safe alternative instead of a fragile workaround.
- Commit freely once the user is happy ("делай смело коммит"); keep the repo clean of junk files.
- The loop (2026-10-27, superseding "rebuild only when asked"): see each change on a copy of the
  debug binary with `shot.sh` and demo data, never by rebuilding the app; commit; build the app
  at once with `scripts/bundle-macos.sh` (it quits nothing); then ask them to restart Bench —
  they quit and reopen it themselves. Never quit their Bench from inside it.
- One app: `/Applications/Bench.app`, rebuilt in place by `ship.sh`, signed with the Apple
  Development certificate so permissions are asked once. (An app per commit filled System
  Settings → Privacy with dead entries; a parked second app and the Dev build were dropped too: one app, and a bad change is fixed
  by the next one.)
- When an approach turns out heavier than expected (a browser engine, a big runtime), stop and
  say so before shipping it; the user prefers dropping a feature to carrying weight.
- **The composer is part of the thread** (2026-10-24): the messages' font size, no buttons
  (⌘Enter, ⌘⇧D, drag-and-drop), no scroll of its own, there while history loads; the thread
  scrolls past its end like a code editor. The mic lives in the status bar.
- **The sidebar is a tree** (projects and their threads, nothing else): no "New session"
  button; "+" on a project is a new thread (⌘N), "New project", the last row of the projects, opens its page in the center.
- **Fold strips (Background, Changed files) are as tall as a source chip.**
- **One size in the thread** (2026-10-24): the thread is one document in one font size — the
  messages' (`text_base`). Step summaries ("Sending… · 17s", "Done · N actions"), step rows,
  site labels, file and fold strips, source chips, plan and asks: all `text_base`, set apart by
  color (muted) only. Code stays monospaced. "это единый документ. Он одним шрифтом идет."
- **One size in the bars**: header title, settings tabs, every status bar item
  are the sidebar's group-name size (`text_sm`, buttons `.small()`, icons `.small()`).
- **No centering, no max width** for the thread: full width from the left, like an editor.
- **Primary buttons in the accent** (`button_primary = primary` in `theme.rs`), never the kit's black.

