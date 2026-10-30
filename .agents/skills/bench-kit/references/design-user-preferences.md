# What the user wants (from many rounds of feedback)

Dated where it changed. The person's later decisions win over earlier ones: a rule marked
*superseded* is history, kept so it is not brought back.

## Look and feel (final, 2026-10-28)

The person: "этот дизайн основной … финальный и исчерпывающий". Keep it; a new piece is made
in the same voice. The rules are `design-look-rules.md`, the values `design-system.md`, the
places `design-layout.md`.

- **A Mac app.** Xcode, Finder, Mail, Messages, System Settings are the references; take their
  concrete values (sizes, weights, radii), never a spec of one's own — "spec-driven" tweaks made
  it worse, Apple's numbers made it right. Everyone knows that UI: no learning a new one.
- **One light and one dark half**, following macOS; no theme, accent or size to choose.
- **A classic chat**: the person's messages as grey bubbles, no avatars, the composer pinned at
  the bottom (Claude's one-line field: plus, text, mic, Send — the three the same round size).
  The document look stays in code (`src/thread/look.rs`), not a setting.
- **The sidebar as a Mac source list**: 13pt one-line rows, heads 11 semibold in their own
  case (Xcode has no heavy sections), a selected row a soft grey pill whose text keeps its
  colour, marks as Finder tag dots, the pin faint after the title, "New project" at the foot.
- **Mac controls**: segmented controls (Chat | Code, Light · Dark · System), switches for
  on/off, a pull-down for the project, navigator glyphs for Files | Threads, a completion list
  for `/`, the menu bar with every shortcut.
- **Bars balanced**: the title bar 40 across the window with the fold right after the traffic
  lights; the status bar 29 in one muted tone; facing bars of one height with one rule.
- **Widgets in Apple's way**: blocks as grouped boxes with soft hairlines and a card's corner;
  a work's steps as a grouped list (a verb column, a file's name before its folder); a
  question a soft tinted card, answered a quiet line; changed files as Xcode's list.
- **Insets**: one rhythm, measured to the point; a thread in a narrow dock takes narrow sides.
- **Code**: Menlo 12 at Zed's leading (1.618).
- Icons: Lucide, monochrome, small.
- Text in single-line places is truncated, never wrapped.
- Nothing jumps on load; nothing changes size on hover or selection.
- Media in the chat itself (images, video, audio, Markdown, diagrams, formulas), the lightest
  implementation (`build-media.md`).

*Superseded* (do not bring back): Zed's look and One Light/One Dark; theme and accent pickers;
avatars and the composer as the thread's last row; the rail of ticks instead of a scrollbar;
"one size in the thread" (14 for everything); CAPITAL group heads; tinted rows with a stripe for
marks; a compact sidebar option; the thread full width from the left; the mic, attach and Stop
in the status bar; a header "New window" button; the task list (removed 2026-10-28, Things
is separate).

## Behaviour

- English UI throughout.
- No dialogs for simple things: defaults + inline editing + arm-to-confirm for deletes.
- No status bar in the chat (2026-10-28, after Claude Desktop): the agent is the sidebar's
  foot; the thread's project, repo and meters the line under its composer; the title bar's right
  end only Chat | Code. The Code view keeps an IDE status bar under its documents only. The thread's own readouts (context, speed) sit in
  the title bar by its repo and open detail popovers on click (stats, speed chart).
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
- Dictation (the composer's mic, ⌘⇧D): the mic stays the mic while it records, a small level
  bar on its edge; the typed text is never hidden (a thought is dictated in pieces between
  typing); a spinner while transcribing.
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
  (`src/app/SPEC.md`).
- The IDE is built in (the Explorer, Code): no external editor is referenced or needed.

- **The Bench aggregates; data stays native** (servers in `~/.ssh/config`). The user rejected a JSON store, then SQLite, then a
  third-party app (Things) as the store, then an MCP built into the Bench: the agents' MCP lives
  in the agent, the Bench only shows and edits (`build-native-data.md`). *Updated 2026-10-24:*
  threads now live in `~/.bench/bench.sqlite` and Pi's store, owned by the agent (`agent/`),
  which Bench reads and writes through it; still no store of Bench's own. Tasks are not
  Bench's (removed 2026-10-28; the person keeps them in Things).
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
- One app: `/Applications/Bench.app`, rebuilt in place by `scripts/bundle-macos.sh`, signed with the Apple
  Development certificate so permissions are asked once. (An app per commit filled System
  Settings → Privacy with dead entries; a parked second app and the Dev build were dropped too: one app, and a bad change is fixed
  by the next one.)
- When an approach turns out heavier than expected (a browser engine, a big runtime), stop and
  say so before shipping it; the user prefers dropping a feature to carrying weight.
