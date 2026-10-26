# What the user wants (from many rounds of feedback)

## Look and feel

- Reference: the Zed editor. Flat, square, monolithic, dense but breathing. No rounded
  corners, no shadows, no gradients, no "cards floating on a background". Slight rounding of the
  search field, primary button and message blocks was tried: "отвратительный вид", removed.
  Only controls that hang inside a field (composer Dictate / Send) are round circles.
- Light theme by default; dark is optional (Light / Dark / System + accent + S/M/L size).
- Fonts: happy with 14 UI / 12 code; adjust only by small steps, never big redesigns. Chat text
  (answers and own messages) was `text_sm` like session titles ("идеально"), then on
  2026-10-24 one size up, `text_base`, with a bit more inset: the middle is what is read (Zed's
  Zed does this). Disliked there: slight rounding, mixed fonts between blocks.
- Syntax: One Light (One Dark in dark mode), the same in every editor and diff.
- Icons: Lucide, monochrome, small. Liked them a lot.
- Spacing is the most frequent complaint. Wants one rhythm ("like a step / golden ratio"):
  equal insets on all sides of a block, facing bars of equal height, rules that line up across
  panels, no 1px "steps" between neighbouring borders. Points out 1–3 pt differences — measure.
- Text in single-line places is truncated with an ellipsis, never wrapped.
- Wrapped titles: tight line height (1.25), not looser than the gap to the line below.
- No scrollbars. Navigation aids are quiet (a rail of ticks).
- Hover-only actions; nothing jumps or changes size on hover/selection (color only).
- Nothing jumps on load either: opening a chat with images/diagrams must not shift the view
  after the first frame.
- One tone per bar: every label and icon in the status bar the same muted color.
- Q&A reads as strips: the user's message a band from edge to edge, the agent's side under it,
  both at one inset (now 2rem on every side — it was 16/32 "forum thread" once, then 24; see
  "Insets in small steps"). Then (2026-10-24) the band went: avatars in a left gutter, as in
  Zed; the sidebar's right rule went too ("олдскульно") and the chrome got darker.
- The user's own messages are selectable/copyable like the answers.
- Media in the chat itself — images, video playing in place, audio, Markdown, diagrams,
  formulas; anything else opens in its default app on a click. All with the lightest possible
  implementation (no browsers), see `references/build-media.md`.
- Color labels: thin stripe + light tint of the row background, for quick scanning.
- Working items (a session whose agent runs) show a moving highlight over their title
  (`ShimmerText`, follows the text color) besides the "working…" meta, so they are found at a glance.

- Sidebar projects as a file explorer: icon + name + chevron/"+" on the right, rows on an
  indent guide; no counts, no horizontal rules (see layout-recipes → Sidebar).
- Tabs and lists that can overflow scroll sideways rather than squeeze; code editors don't
  soft-wrap in narrow panels (horizontal scroll).

- **Every bar the header's height, every crossing rule one line.** The user spots a 1px step
  between the composer and "New session" and calls it "бесит". All bars 36; the composer's
  buttons shrank (30 → 26) so the bar could, and the waveform with them.
- **No text-size setting** (removed 2026-10-24): one size, tuned; ⌘+ / ⌘− still zoom.
- **Insets in small steps.** "Чуточку больше" means one step (16 → 20 → 24 over two rounds),
  the same number on every side and in every place of the area (question, answer, composer, task
  list, task card). A big app's generous padding (Claude Desktop) is the direction, not the target.
- **Mode toggles over the sidebar**, in the strip beside the window buttons (Tools, then Tasks);
  the status bar's right corner holds only New window and Settings. *Superseded 2026-10-24:*
  there are no modes. *Then 2026-10-25:* the sidebar holds only projects and their threads; Tasks,
  servers, files, tools and the browser are inserts put into a thread with `/` (`build-insert.md`).
  The status bar rule stands.
- **Cards are frames, not fills**: code blocks, audio, video, Changed files — a 1px frame, no
  background; header lines all the same height. A darker code background read as "тёмный вид".
- **Code in answers has a header**: language left, Copy right, like every AI chat.
- **Fields can read as text**: "Name a group or Select folder" on one line, the field without a
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
- Remembers state: last backend and open session per backend; panel toggles; prefs.
- Things that belong to the backend live there as files the user can edit elsewhere (prompts as
  `prompts/*.md` in the agent dir or the project's environment, skills, SYSTEM.md) — not in
  app-private storage.
- Multi-window rather than tabs: one window = one independent context.
- Git when needed, never forced: the repo and branch are detected from the thread's folder or
  edited files and shown as a label; no git actions of Bench's own (branches per thread were
  dropped). Chats with no files show nothing git-related.
- Dictation: show a live waveform in place of the text field while recording, with equal gaps
  to the left edge and to the button; the mic button turns into a filled round Stop (■) — not an
  animated "waveform" icon; spinner while transcribing.
  Send while recording (or transcribing) = stop + recognize + send in one press; Send is shown
  active (filled) during recording.
- Nothing slow starts by itself: version checks etc. run from an explicit button.
- Placeholders are misleading: hide model / effort / approvals until the real values arrive.
- Restart / Stop must actually work and say why when they can't.
- Standard shortcuts everywhere (⌘S save, ⌘N new, ⌘⇧N new window, ⌘, settings, Esc cancels
  recording/forms…), with the shortcut in the button's tooltip. Esc must still reach a focused
  terminal (vim) — a root `capture_key_down` takes only ⌘ combos and Esc in specific states.
- One source of truth, no app-private copies of what the system already stores: servers live in
  `~/.ssh/config` (app fields as `# folder:` / `# path:` comments), keys as `IdentityFile`,
  never passwords.
- Nothing installed on servers, no personal data or tokens left there; agents stay local.
- Lightweight first, the heavy tool one click away: a file opens in the thread (the `/files`
  insert); the whole project opens in Zed from the project's ⋯ menu (only where it has a folder).

- **The Bench aggregates; data stays native** (servers in `~/.ssh/config`). The user rejected a JSON store, then SQLite, then a
  third-party app (Things) as the store, then an MCP built into the Bench: the agents' MCP lives
  in the agent, the Bench only shows and edits (`build-native-data.md`). *Updated 2026-10-24:*
  tasks and sessions now live in `~/.bench/bench.sqlite`, owned by the agent (`agent/`), which
  Bench reads and writes through it; still no store of Bench's own.
- No iPhone notifications from tasks (no alarms); back-dated completion is welcome; tags matter
  more than lists or priorities.
- After moving data, the scheduled reports must keep working: check every cron job and skill
  that touched the old place.
- Groups of sessions without a real project are wanted ("an audit", "a research" of 3–5 deep
  researches): typed by name in the new session, no folder picking.

## How to work with the user

- The user describes problems by voice in Russian, often several at once, mid-task: acknowledge each
  briefly, keep going, address all of them, report per item.
- Wants honest status: what was checked, what only builds, what needs the user's click.
- "Don't hack": prefer supported APIs; if something isn't supported (e.g. downgrading the
  backend), say so and offer the safe alternative instead of a fragile workaround.
- Commit freely once the user is happy ("делай смело коммит"); keep the repo clean of junk files.
- Rebuild only when they ask (2026-10-26): commit each change and keep going; they ask for one
  `ship.sh` at the end of a batch. A rebuild per step costs them a restart each time and gains
  nothing. Never quit their Bench outside `ship.sh` (they develop Bench from inside Bench).
- One app per commit (`Bench.<hash>.app`) filled System Settings → Privacy with dead entries;
  hence two apps on fixed paths, signed with the Apple Development certificate.
- When an approach turns out heavier than expected (a browser engine, a big runtime), stop and
  say so before shipping it; the user prefers dropping a feature to carrying weight.
- **The composer is part of the thread** (2026-10-24): the messages' font size, no buttons
  (⌘Enter, ⌘⇧D, drag-and-drop), no scroll of its own, there while history loads; the thread
  scrolls past its end like a code editor. The mic lives in the status bar.
- **The sidebar is a tree** (projects and their threads, nothing else): no big "New session" button; "+" on a project or in the top strip.
- **Fold strips (Background, Changed files) are as tall as a source chip.**
- **One size in the thread** (2026-10-24): the thread is one document in one font size — the
  messages' (`text_base`). Step summaries ("Sending… · 17s", "Done · N actions"), step rows,
  site labels, file and fold strips, source chips, plan and asks: all `text_base`, set apart by
  color (muted) only. Code stays monospaced. "это единый документ. Он одним шрифтом идет."
- **One size in the bars**: header title, settings tabs, every status bar item
  are the sidebar's group-name size (`text_sm`, buttons `.small()`, icons `.small()`).
- **No centering, no max width** for the thread: full width from the left, like an editor.
- **Primary buttons in the accent** (`button_primary = primary` in `theme.rs`), never the kit's black.

