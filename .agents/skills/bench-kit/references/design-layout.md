# Layout recipes

All snippets use `use gpui_kit::*; use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex}; use gpui_kit::prelude::FluentBuilder as _;`.
Sizes are `bench_ui` names (`crates/ui/src/metrics.rs`); the numbers below are what they are now.

## Window skeleton

```
┌ sidebar 280 ─────┬ header 36 ─────────────────────────────────┐
│ (lights)    + ◧  │ title …                  repo · branch  ▢  │
│ search band 35   ├────────────────────────────────────────────┤
│ ▣ Host     ⋯ + ⌄ │ the thread: messages, work, inserts, cards │
│ │ threads…       │                                            │
│ ▤ project  ⋯ + ⌄ │                                            │
│ │ threads…       │ ◉ composer (the thread's last row)         │
├──────────────────┴────────────────────────────────────────────┤
│ status bar 29: gateway · model · context · effort · approvals … ⌘ 📎 🎤 ⚙ ■ │
└───────────────────────────────────────────────────────────────┘
```

`src/workspace/layout.rs` `render_layout`: `h_resizable` with the sidebar a fixed `resizable_panel`
(`sidebar_w`, 200–480, its width kept) and the main column; the status bar under both
(`render_statusbar`). The column divider is `quiet_divider`: transparent at rest, an accent line
on hover. A whole project opens in Zed from its menu. The title bar is transparent: the sidebar starts with a 35px strip beside the traffic
lights (`render_sidebar_top`: + New project, Hide sidebar ⌘\); the header (36) tops the main
column.

## Facing bars (the alignment the person notices)

```rust
/// The header's 36 less the one rule a bar draws: 35 at the 14px font.
pub fn bar_h(cx: &App) -> Pixels { cx.theme().font_size * 2.5 }
```

All bars are the header's height, and every rule crossing the window is one line:
- Sidebar top strip `h(35)` + the search band's top = the header's 36: the two rules are one
  pixel row.
- Search band: **always there**, `bench_ui::search_band` (`crates/ui/src/bars.rs`), `h(bar_h)`,
  transparent on the sidebar's color, no frame; ⌘F focuses it (`app/keys.rs` `open_search`,
  which also brings a hidden sidebar back).
- **Hiding the sidebar** (⌘\): `render_layout` leaves the panel out (the flag is in the
  `layout_synced` shape key) and the header gets a 36px corner on its left (`title_bar` bg,
  `border_b`, `pl(78)` for the window buttons, PanelLeft to show it again).
- No "New session" button anywhere: "+" on a project is a new thread (⌘N), "+" in the top strip
  a new project.
Measure it (`build-verification.md`): the rules' pixel rows must be equal, not "about".

## Sidebar

Only projects (`src/sidebar/mod.rs`, order and acts in `src/sidebar/items.rs`). Order
(`sidebar_groups`, the order ↑ ↓ walk): this Mac's **Host** first with the threads of no
project (Telegram's, cron's), then each project — pinned projects first, pinned threads first
inside. A server's Host (its agent's own environment) is followed by that server's projects.

**A project head looks like a file explorer's folder**: a 16px icon of its kind
(`head_icon_in`: `server` a server's Host, `folder-git-2` a git folder, `folder`, `layers` no
folder), the name `text_sm` medium, then on the right — on hover — "⋯" and "+"
(`head_plus_slot`), and the fold chevron always last (`chevron-right` folded, `chevron-down`
open). **No counts, no rules between groups** (both tried and rejected). Click on the head folds
it; buttons inside call `cx.stop_propagation()`. Folds persist.

Thread rows: title at `SESSION_TITLE` (14), `py(5)` (compact `py_1p5`), `pr_3`, a 2px left
stripe; under a project they hang on an indent guide (`nest_guide`) with text at `nest_pl`.

```rust
h_flex().id(("session", i)).group(group)          // unique group name per row!
    .pl(nest_pl(cx)).pr_3().py(px(5.)).border_l_2()
    .border_color(stripe.unwrap_or(if selected { theme.primary } else { theme.transparent }))
    .when_some(stripe, |d, c| d.bg(Hsla { a: if selected { 0.22 } else { 0.12 }, ..c }))
    .when(selected && stripe.is_none(), |d| d.bg(theme.primary.opacity(0.12)))
    .hover(move |h| match stripe { Some(c) => h.bg(Hsla { a: if selected { 0.30 } else { 0.20 }, ..c }), None => h.bg(theme.sidebar_accent) })
```

**The menu** is one function, `item_menu` (`sidebar/menu.rs`), shown two ways: the hover "⋯"
(`Button…dropdown_menu`, wrapped in a div that stops `MouseButton::Left` so the row doesn't open)
and a right click (`.context_menu(item_menu(…))`). Items: a project — Environment, Open in Zed
(with a folder; a server's over SSH), separator; then Pin/Unpin (not on this Mac's Host), Rename
(threads; a project is renamed in its Settings), the color swatches
(`PopupMenuItem::element`), Delete… / Remove project… (not on a Host). Also by keys:
⌫ ⌫ deletes the selected thread (asked once), a project shows Remove · Cancel in its head;
rename is in place (the title becomes a field); ↑ ↓ walk the rows. "Open in new window" is not
a row's: New window is the header's.

## Header strip

`h(36).bg(title_bar).border_b_1().pl_4().pr_2().gap_1()` (`titlebar/mod.rs` `render_header`): the
title (`flex_1 min_w_0 text_sm` medium, truncate), the project chip (repo · branch when the
thread works in git, `render_project_chip`), then `render_header_tail`
(`src/inserts/terminal/form.rs`): New window ⌘⇧N as a `bench_ui::icon_button`. Tasks, servers,
files and tools are inserts (`build-insert.md`), never header switches. The gear's page has the
title "Settings" and a close ×; its tabs (Agents, Presets, Appearance) are on the page
(`src/settings/mod.rs`).

## Status bar

`STATUS_H` 29 with its rule, `bg(status_bar)`, `text_sm` (`src/statusbar/mod.rs`). Left: the
gateway of the project's machine (always there; its state, log, Reconnect, Restart inside), the
fallback in red when it answers, then model, context, effort, approvals. Right, icons only:
background terminals (if any), attach (paperclip), dictate (mic, with a level bar while
recording), the gear, and **Stop** last, at the window's edge, while a run is live (⌘. too).
A picker is a ghost Button **with the icon inside it** and a `dropdown_menu` of checked items:

```rust
Button::new(id).ghost().small().icon(Ico::Cpu.icon()).label(label).dropdown_menu(move |menu, _, _| {
    items.iter().fold(menu, |m, (text, checked, f)| m.item(PopupMenuItem::new(text.clone()).checked(*checked).on_click(…)))
})
```

Readouts with more detail open a `Popover` on click (stats, the speed chart) — see
`ui-gotchas.md`. Hide a picker until its data is loaded (no placeholder values: a `ready` flag,
reset on reconnect). Every label and icon `muted_foreground`: the bar is one tone.

## Composer

**Part of the thread**: its last row (the list's tail item, `src/thread/composer.rs`
`render_composer`), there from the start, scrolling with the thread. No rule above it, no
background of its own.

The row is a message's: `pl(CHAT_LEFT) pr(CHAT_RIGHT) py(TURN_Y)`, gap
`CHAT_TEXT − CHAT_LEFT − AVATAR` (14), `items_start`, `text_base` — the person's avatar, then a
`Textarea::large()` pulled back by its own padding (`ml(-12) my(-10)`) so its first line sits
where a message's does, leading `rems(LEAD)`. It grows with the text and never scrolls itself.
**No buttons**: ⌘Enter sends (Enter is a new line), ⌘⇧D dictates, files are dropped or pasted
(⌘V of a screenshot or Finder files). Stop and the mic are the status bar's. **No placeholder.**
A new thread is an editor's empty file: the caret alone, focused, no avatar until the first
message. The caret is 2px (`vendor/gpui-base`, `CURSOR_WIDTH`).

**The room past the end**: the list scrolls past its last row by exactly as much as puts the
composer, scrolled to the end, where it stands in a new thread (`src/thread/mod.rs`
`room_past_end`; `build-thread.md`).

**The `/` list** (`render_slash`): under the composer's line, or above it when the window has no
room below (the list's own height decides). Enter / Tab pick, ↑ ↓ move, Esc closes
(`src/thread/slash.rs`, seen first at the root).

**The thread's width**: the window's whole width from its left edge, as an editor's text — no
cap, no centering.

## A project's Settings, the gear

Two settings pages, one look: the gear (the app's, `settings_tabs` on top) and a project's
Settings (`open_project_settings`, `Center::Project`, `src/env/page.rs`): groups under
`settings_heading`, fields in `settings_field` rows, Save / Revert under them once something
changed (`form_actions`). What a project contains — files, soul, prompts, skills, MCP and
language servers, jobs — is the Explorer (`docs/explorer.md`): a tree in the sidebar
(`tree_row`, `tree_heading`), documents in the center with tabs (`doc_tab`, `ide_head`), a file
beside in the right dock; documents fill their pane and scroll inside, as in an IDE. A section is
a `Section` in `src/env/` with its `Home`.

## The thread's column

Every row (`src/thread/message.rs` `speaker_row`) is `h_flex().pl(CHAT_LEFT).pr(CHAT_RIGHT)
.py(TURN_Y).gap(14).items_start().text_base()`: a 24px avatar (`bench_ui::avatar`: the person a
dark disc `foreground 78%` with UserRound; the agent a solid `primary` disc with Bot — both
solid, a pale disc reads smaller), then the text column `flex_1 min_w_0`. A row following the
agent's work keeps the avatar's room empty. Any two rows are `2 × TURN_Y` (28) apart. Everything
that lines up with the text starts at `CHAT_TEXT` (66) and ends `CHAT_RIGHT` (66) from the
right: the composer's text, chips, inserts, cards, the plan — measure that they start within
0.5pt (`build-verification.md`).

Then: the work row ("N actions · time · Done", `src/thread/work.rs`; the live label a
`ShimmerText`; steps unfold into their arguments and output), the Markdown answer split into text
/ files / diagrams (`build-media.md`), and hover actions (copy, read aloud, regenerate on the
last) floating over the answer's top-right corner (`absolute().top(TURN_Y + 3).right_6()`, on the
page's background) — never a row of their own.

Code blocks in an answer are a block: a rounded 1px frame, `block_head` with the language on the
left and Copy on the right, the highlighted code under it, **no fill** (`render_code`,
`src/media/view.rs`; recipe `ui-content.md`).

**The rail** instead of a scrollbar (`src/thread/rail.rs`): absolute in the right gutter,
vertically centered, one 10×2 tick per message of the person (pitch 3–8), the one being read in
the accent, the others muted 35%; hover changes color only and shows the message as a tooltip; a
click brings it to the top (`list.scroll_to(ListOffset { item_ix, offset_in_item })`). Shown
with ≥ 2 messages.
