# Layout recipes

All snippets use `use gpui_kit::*; use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex}; use gpui_kit::prelude::FluentBuilder as _;`.
Sizes are `bench_ui` names (`crates/ui/src/metrics.rs`, `design-system.md`).

## Window skeleton

```
┌──────────────────────────────── title bar 40, across the window ─────────────────────────┐
│ ● ● ●  ◧ │ Thread title  /  second thread ×     38k/131k · storefront · main  [Chat|Code] │
├ sidebar 280 ─────────────┬───────────────────────────────────────────────────────────────┤
│ ✎ New thread        ⌘N   │         the thread, a centred column (≤ 860)                  │
│ ⊞ New project            │                           ┌ the person's bubble ┐             │
│ 🔍 Search           ⌘K   │  8 actions · 3:17 · Done                                      │
│  Publish the release…    │  the answer …                                                 │
│ Storefront               │                                                               │
│ (Launch the spring …)    │  ┌ + Write to the agent · / for a tool          🎤  (↑) ┐     │
│──────────────────────────│  └──────────────────────────────────────────────────────┘     │
│ ● This Mac  1 working  ⚙ │                                                               │
└──────────────────────────┴───────────────────────────────────────────────────────────────┘
```

`src/workspace/layout.rs`: the title bar (`render_window_lead` + `render_header`) over
`render_layout` (the docks around the center); no status bar, as Claude's window; ⌘K's list
over everything while open (`sidebar/search.rs`). The menu bar's
actions are on the window's root (`app/menu.rs` `listen`).

## Title bar (`src/titlebar/mod.rs`)

`TITLE_BAR_H` 40 across the window. Left: room for the traffic lights (`WINDOW_BUTTONS_W`), the
sidebar's fold (⌘\), a short rule (`BAR_SEPARATOR_H`), then the title at `BAR_TITLE` semibold.
A second thread beside the open one adds its title after a faint "/", muted medium, and its ×
(no head of its own over the dock). Right: the thread's meters and repo chip at `BAR_TEXT`, then
the window's toggles (`tail.rs`: running terminals ⌨, the thread beside ✦ ⌘J in Code, the
terminal ▭ ⌃`), then `mode_switch` Chat | Code (⌘E) at the window's corner. Settings and the new project's page put
their title and a close × here.

## Sidebar (`src/sidebar/`)

- **Actions** (as Claude's "New" and "Search"): `bench_ui::nav_row` rows — New thread (⌘N),
  New project, Search (⌘K, also ⌘F) — an icon in the text's column, the shortcut faint on hover.
  Search opens a list over the window (`search.rs`, `bench_ui::quick_frame`, as ⌘P's): recent
  threads and the two actions; typed, threads and projects by name, then threads by message.
- **Groups**: Host first (the threads of no project), then the projects, pinned first. A
  group's head is `SIDEBAR_HEAD_TEXT` semibold muted in its own case, then its tag dot and the
  Telegram glyph; `group_chevron` on hover; a click folds it. `GROUP_GAP` between groups.
- **Thread rows** (`thread_row.rs`): one line at `SIDEBAR_TEXT`, set in by `SIDEBAR_ROW_INSET`,
  `rounded_md`, its text at `SIDEBAR_TEXT_X` (flush with the head). Selected: `sidebar_accent`;
  hover `list_hover`; the text keeps its colour. After the title: the tag dot, a faint pin when
  pinned; on the right "needs you" in the accent while it waits, "paused" or the branch faint.
  A working thread's title shimmers. "⋯" on hover = right click (`item_menu`).
- **Foot** (the left dock's, under the threads or the Explorer; `agent.rs`), as Claude's account
  row: `SIDEBAR_FOOT_H` with a hairline over it; the open thread's agent — where it runs ("This
  Mac"), its dot, "N working" muted — whose popover is a Mac popover: sections edge to edge with
  hairlines, 13pt lines with state dots, 11pt semibold section titles, the log at 11; the gear
  (⌘,) at the right end.

There is no status bar. An open file's caret, indent and language sit at its tab bar's right
end (`explorer/pane/cursor.rs`).

## The thread (chat look, `src/thread/look.rs` `Chat`)

- **Column**: every row centred in `THREAD_MAX_W`, sides `CHAT_LEFT` (in a dock, narrower than
  `THREAD_NARROW_W`, `CHAT_LEFT_NARROW`). The overlay scrollbar of a Mac; "Jump to latest" a
  floating pill (popover ground, shadow) over the composer.
- **The person**: a grey bubble on the right (`BUBBLE_*`), no avatar.
- **The work**: one line "N actions · time · Done" at `THREAD_META`; opened, its steps are a
  grouped box (`foreground` 3%, `rounded_lg`, `p_1`) of `LIST_ROW_H` rows: the glyph in its
  column, the verb in a `STEP_VERB_W` column, muted, the target in the text's colour (a file:
  its name, then its folder faint); the agent's own words in the text's colour, thoughts faint;
  a row lights up whole on hover and unfolds its arguments and output in a mono box.
- **Blocks** (inserts, code, processes, changed files): `block_frame` — the page's ground, a
  hairline at `border` 70%, `rounded_lg`; `block_head` with no fill, `THREAD_META`.
- **Cards in the work**: the plan; a question is a soft tinted card (accent 4% ground, accent
  50% hairline) with choices at the cards' size, answered it becomes a quiet line; the
  agent waiting while the person is elsewhere: an amber line over the composer.
- **Changed files**: Xcode's source-control list — file icon, name, the repo-relative folder
  faint, counts in mono; ten, then "Show all N".

## Composer (`src/thread/composer/input.rs`)

Pinned under the list, in the thread's column, `COMPOSER_CARD_PB` over the window's edge. One
line of a field, as Claude's: the card (`COMPOSER_CARD_R`, a hairline, no shadow) holds
`round_icon_button` + on the left, the text in the middle, the mic and Send on the right — all
three `SEND_BUTTON` round, on the text's last line; the text grows upward. Files to send ride
over the text as chips. Send: the accent with words, a faint disc without, a dark disc with a
square (Stop) while the agent works. Placeholder "Write to the agent · / for a tool".

**The `/` list** (`render_completions`): a Mac completion list right over the field and as
wide, `COMPLETION_GAP` above it, `rounded_lg`, popover ground and shadow, rows
`COMPLETION_ROW_H` at 13: the word, what it is muted, its kind faint at the end. One selection,
the accent with white text, that follows the pointer and the arrows. The terminal and the
browser first, then every utility by its own word.

## The IDE (Code, `src/explorer/`)

- **The Explorer's bar** (`tree/view.rs` `explorer_picker`): pinned over the tree, `bar_h`
  tall with the rule the documents' tabs have, so the line crosses the window. The project is a
  pull-down (`BAR_TITLE` semibold + chevron); then two navigator glyphs — Files, Threads — the
  shown one in the accent; a short rule; Refresh and ⌘P.
- **Threads** in the left dock: the project's threads as sidebar rows; one clicked opens beside
  the document.
- **Tabs** (`doc_tab`): Xcode's — the open one on white, the others on the chrome with a rule
  between. **Heads** (`ide_head`): no fill, the title semibold, its path faint.
- **Editor**: mono 12 at `CODE_LEAD`.
- **Terminal dock** (`src/inserts/terminal/panel.rs`): the editor's ground; its bar no fill with
  a rule under it; several terminals a segmented control (`term_tab` in a `segment_track`).

## Settings (`src/settings/`, `src/env/page.rs`)

A centred column (`SETTINGS_W`), as a Mac settings pane: `settings_tabs` on top (the gear),
`settings_heading` per group (13 semibold), `settings_field` rows with a hairline under each;
a choice of a few is `segmented`, on/off a `Switch`, actions `form_action` at 13. The new
project's page is a column of `PAGE_W` with `choice_card`s.

## The menu bar (`src/app/menu.rs`)

A Mac app's: Bench (Settings…, Services, Hide, Quit), File (New Thread, New Project…, New
Window, Open File…, Find in Project…, Save, Close Tab), Edit (the system's text items, Find
Threads…), View (Chat or Code, Sidebar, Thread Beside, Terminal, Problems, text size, Full
Screen), Thread (Stop, Dictate, Send Selection, Delete Thread…), Window, Help. Each item is an
action bound to its key there; a new shortcut goes there too.
