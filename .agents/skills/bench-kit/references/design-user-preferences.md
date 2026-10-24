# What the user wants (from many rounds of feedback)

## Look and feel

- Reference: Zed editor / Delta. Flat, square, monolithic, dense but breathing. No rounded
  corners, no shadows, no gradients, no "cards floating on a background". Slight rounding of the
  search field, primary button and message blocks was tried: "отвратительный вид", removed.
  Only controls that hang inside a field (composer Dictate / Send) are round circles.
- Light theme by default; dark is optional (Light / Dark / System + accent + S/M/L size).
- Fonts: happy with 14 UI / 12 code; adjust only by small steps, never big redesigns. Chat text
  (answers and own messages) at `text_sm`, like session titles — "идеально".
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
  both at one inset (now 24 on every side — it was 16/32 "forum thread" once, then one number;
  see "Insets in small steps").
- The user's own messages are selectable/copyable like the answers.
- Media in the chat itself — images, video playing in place, audio, Markdown, diagrams,
  formulas; anything else opens in its default app on a click. All with the lightest possible
  implementation (no browsers), see `references/build-media.md`.
- New desktop apps: the same stack and the same look as Hermes GPUI, only the icon differs
  (`references/build-new-app.md`).
- Color labels: thin stripe + light tint of the row background, for quick scanning.
- Working items (a session whose agent runs) show a moving highlight over their title
  (`ShimmerText`, follows the text color) besides the "working…" meta, so they are found at a glance.

- Sidebar groups as a file explorer: kind icon + name + chevron/"+" on the right, rows on an
  indent guide; no counts, no horizontal rules (see layout-recipes → Sidebar).
- Tabs and lists that can overflow scroll sideways rather than squeeze; code editors don't
  soft-wrap in narrow panels (horizontal scroll).
- "Maximize" is a layout mode, not an overlay: the maximized panel takes the places of the
  sidebar (same width) and the main area, toggled back with the same button/shortcut. An
  immersive view needs no labels saying where it is.

- **Every bar the header's height, every crossing rule one line.** The user spots a 1px step
  between the composer and "New session" and calls it "бесит". All bars 36; the composer's
  buttons shrank (30 → 26) so the bar could, and the waveform with them.
- **Insets in small steps.** "Чуточку больше" means one step (16 → 20 → 24 over two rounds),
  the same number on every side and in every place of the area (question, answer, composer, task
  list, task card). A big app's generous padding (Claude Desktop) is the direction, not the target.
- **Mode toggles over the sidebar**, in the strip beside the window buttons (Tools, then Tasks);
  the status bar's right corner holds only New window and Settings.
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
- Remembers state: last backend, profile and open session per backend; panel toggles; prefs.
- Things that belong to the backend live there as files the user can edit elsewhere (prompts as
  `prompts/*.md` in the profile, skills, SOUL.md) — not in app-private storage.
- Multi-window rather than tabs: one window = one independent context.
- Git when needed, never forced: project detected automatically from the session's folder or
  edited files; a chat can get its own branch (worktree); merge/delete from a chip. Chats with no
  files show nothing git-related.
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
- Lightweight first, the heavy tool one click away: open in the built-in mini IDE first, Zed
  from inside it.

- **The Bench aggregates; data stays native** (tasks in Apple Reminders, servers in
  `~/.ssh/config`, sessions in Hermes). The user rejected a JSON store, then SQLite, then a
  third-party app (Things) as the store, then an MCP built into the Bench: the agents' MCP lives
  in Hermes, the Bench only shows and edits (`build-native-data.md`).
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
- Always build the commit's app before saying a change is ready — a new `dist/Bench.<commit>.app`
  next to the running one (never quit theirs: they develop Bench from inside Bench). They switch
  the Dock icon themselves and keep the older builds for a while to roll back.
- When an approach turns out heavier than expected (a browser engine, a big runtime), stop and
  say so before shipping it; the user prefers dropping a feature to carrying weight.
