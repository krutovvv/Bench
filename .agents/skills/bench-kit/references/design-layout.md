# Layout recipes

All snippets use `use gpui_kit::*; use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex}; use gpui_kit::prelude::FluentBuilder as _;`.

## Window skeleton

```
┌ sidebar 280 ─────┬ header 36 ────────────────────────────┐
│ (lights) 📂 + ◧  │ title …                  [chip] [▢]   │
│ search band 36   ├───────────────────────────────────────┤
│──────────────────│ the thread: messages, inserts in the  │
│ HOST             │ person's rows / settings page         │
│ │ threads…       │                                       │
│ PROJECT  ⋯ +     │                                       │
│ │ threads…       │ ◉ composer (in the thread)            │
├──────────────────┴───────────────────────────────────────┤
│ status bar 28: target ……… speed · context · approvals · effort · model · ⟳ ⚙ │
└──────────────────────────────────────────────────────────┘
```

```rust
v_flex().size_full().child(
    div().flex_1().min_h_0().child(
        h_resizable("layout")
            .child(resizable_panel().size(px(280.)).size_range(px(200.)..px(480.)).child(sidebar))
            .child(resizable_panel().child(main)),
    ),
).child(statusbar)
```

There is no right panel (the mini IDE is gone): files open in a thread's insert, a whole project
in Zed from its ⋯ menu. The title bar is transparent (`appears_transparent: true`): the sidebar starts with a 35px
strip beside the traffic lights (its icon buttons on the right: + New project, Hide sidebar); the main header (36px) sits at
the top of the main column.

## Facing bars (the alignment users notice)

```rust
/// The header's 36 less the one rule a bar draws: 35 at the 14px font.
pub fn bar_h(cx: &App) -> Pixels { cx.theme().font_size * 2.5 }
```

All bars are the header's height, and every rule crossing the window is one line:
- Sidebar top strip `h(px(35.))` + the search band's top rule = the header's 36 → the band's
  top rule *is* the header's bottom rule, on one pixel row.
- Search band: **always there**, `h(bar_h)`, transparent on the sidebar's color, no rules, no
  frame (`ui/mod.rs` `search_band`); ⌘F focuses it (`app/keys.rs` `open_search`, which also
  brings a hidden sidebar back). No magnifier toggle — the user had it removed (2026-10-24).
- **Hiding the sidebar**: the last icon in its top strip (PanelLeft, "Hide sidebar ⌘\\",
  after +). Hidden, `render_layout` leaves the left panel out (the flag is part of the
  `layout_synced` shape key), and the
  header gets a 36px corner on its left: `title_bar` bg, `border_b`, `pl(78)` for the window
  buttons, the same PanelLeft to show it again.
- **No "New session" button at the bottom**: the sidebar is a tree (projects as folders,
  threads inside); a new thread is the "+" on a project (⌘N); the "+" in the top strip is a new project.
- A strip under the header that faces the search band (Tasks' tag strip): `min_h(bar_h)`, so
  its bottom rule meets the band's.
- Any "+ New item" row at the bottom of a list: `h(bar_h + 1)`.
Measure it (`build-verification.md`): the rules' pixel rows must be equal, not "about".

## Sidebar rows and groups

Session titles 14px (`SESSION_TITLE` in `sidebar.rs`; 15 was "too big"), rows `py(9)` (compact `py_1p5`). The open chat is tinted with the accent
(`primary 12%`), not one more grey — the user found the all-grey sidebar dull.

```rust
h_flex().id(("row", i)).group(group_name)          // unique group name per row!
    .gap_1().pl(px(14.)).pr_3().py_2()
    .border_l_2().border_color(stripe.unwrap_or(if selected { theme.primary } else { theme.transparent }))
    .when_some(stripe, |d, c| d.bg(Hsla { a: if selected { 0.22 } else { 0.12 }, ..c }))
    .when(selected && stripe.is_none(), |d| d.bg(theme.sidebar_accent))
    .hover(|s| s.bg(theme.sidebar_accent))
    .child(v_flex().flex_1().min_w_0()
        .child(div().text_sm().truncate().child(title))
        .child(div().text_xs().text_color(theme.muted_foreground).child(meta)))   // "branch · 6 messages"
    .child(hover_only_menu_button)
```

Grouping: Pinned (top) → plain items → projects/folders (alphabetical) → other sources. A
single plain group shows no header; a project group always shows its name.

**Groups look like a file explorer** (settled after several rounds; reference: Bench
`ui/sidebar.rs`, `ui/servers.rs`, helpers in `ui/mod.rs`):
- Header row: a 16px icon of the group's *kind* (folder, git repo `folder-git-2`, chats
  `message-square-text`, Telegram `send`, cron `clock`, pinned `pin`), the name in `text_sm`
  medium (not uppercase), then on the right a fold chevron and an always-visible muted "+".
  **No counts** (a "· 2" or a badge was tried twice and rejected as noise), **no horizontal
  rules** between groups (rejected as ugly) — spacing `pt_3` between groups does the parting.
- Rows of a group hang on an indent guide: a 1px `theme.border` line under the center of the
  header icon (`left = 1.5rem − 2.5px` inside a row with a 2px stripe), row text aligned with
  the header's name (`pl = 2.375rem − 4px`, measured). In the group's **last row the guide stops
  where the text does** (`bottom = 0.5rem`, the row's `py_2`), not in its bottom padding.
  Rem-based so S/M/L sizes keep the alignment.
- Click on a header folds the group; the chevron (`chevron-right` folded / `chevron-down` on
  hover when open) sits at the **right**, before the "+". Folded state persists in a JSON file.
- Click handlers inside the header ("+") call `cx.stop_propagation()` so they don't also fold.

Row menu on hover ("⋯", ghost xsmall Button with `dropdown_menu`): Pin/Unpin, a row of color
swatches (`PopupMenuItem::element`), the item's own actions (e.g. "Open files"), separator,
Delete (arms first). "Open in new window" does **not** belong to a row: New window is a header
button that is always there.
Wrap the button in a div with `.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())`
so opening the menu does not also open the row.

## Header strip

`h_flex().h(px(36.)).bg(theme.title_bar).border_b_1().pl_4().pr_2().gap_1()`: title
(`flex_1 min_w_0 text_sm font_weight(MEDIUM) truncate`), then the project chip (repo · branch
when the thread works in git, `render_project_chip`), then 24px icon toggles (`p_1`, muted, active =
accent + `list_active` bg, tooltip with the shortcut, e.g. "New window  ⌘⇧N"; `render_header_tail`,
`ui/servers.rs`). Tasks, servers, files and tools are not switches anywhere: they are inserts
put into a thread with `/` (`build-insert.md`). Item-specific actions (Open in Zed) go to the
item's "⋯" menu, not the header. Settings (the gear) is one page with a plain "Settings" title
and a close ×.

## Status bar

28px, `bg(status_bar) border_t_1 text_xs`. Left: where (target). Right: live
readouts and pickers. A picker is a ghost xsmall Button **with the icon inside it** (so hover
highlights both) and a `dropdown_menu` of checked items:

```rust
Button::new(id).ghost().xsmall().icon(Ico::Cpu.icon()).label(label).dropdown_menu(move |menu, _, _| {
    items.iter().fold(menu, |m, (text, checked, f)| m.item(PopupMenuItem::new(text.clone()).checked(*checked).on_click(…)))
})
```

Readouts with more detail open a `Popover` on click (stats table, speed chart) — see
`references/ui-gotchas.md`. Hide a picker until its data is loaded (no empty dropdowns, no
default values pretending to be real: keep a `ready` flag, reset it on reconnect). All picker buttons `.text_color(muted)` so the bar is one tone.

Row menus: separate unrelated groups with `.separator()` (Pin | colors | Open in new window |
Delete).

## Composer

The composer is **part of the thread**, as in Zed's Delta — not a field docked to the window,
not a textarea: it is the thread's last row (`render_chat`), after the turns and cards, and it is
there from the start — under "Loading history…" while a session opens (you can type at once),
under the welcome block in a new chat. It scrolls away with the thread; "Jump to latest" brings
it back. No rule above it.

The row as a turn's: `pl(CHAT_LEFT) pr(CHAT_RIGHT) py(TURN_Y) gap(14) items_start` — the person's
avatar (`ui::avatar(false, ..)` at `mt(AVATAR_TOP)`, centered on the first line, measured), then
the text at `CHAT_TEXT` (the textarea pulled back `ml(-10)` by its own padding; chips over it).
**The text, placeholder included, is the messages' size** (`Textarea::large()` = `text_base`;
the default Medium is `text_sm`, which the user called too small). It **grows with what is typed
and never scrolls itself** (`auto_grow(1, 100_000)`): pasted code reads like a message, the
thread scrolls. **No buttons** — the user found clip, mic and Send noise: ⌘Enter sends, ⌘⇧D
dictates (the mic button is in the status bar, by New window), files are dropped or pasted. Only
while the agent works a quiet Stop (`chat::icon_button`, ⌘. too). While recording the waveform is
drawn over the field (`absolute().inset_0()`), so nothing moves. **No placeholder** — it is the
thread's next text, not a form field (Delta has none). **A new chat is a code editor's empty
file**: no welcome block, no avatar (`invisible()`, its room kept so nothing moves when the
thread starts), only the focused caret; avatars appear with the first message. The caret is 2px
(`vendor/gpui-base`, `CURSOR_WIDTH`; 1px upstream was easy to lose). A folder for the new chat:
the project's (its "+"); a project without a folder works in one Bench keeps for it.

**Scroll past the end**: the thread's column has `pb(SCROLL_PAST_END)` (240) below the composer,
as a code editor scrolls past its last line, and `pt(CHAT_LEFT)` above the first turn (the top
room is at least the sides').

Keys as in Delta: **Enter is a new line, ⌘Enter sends** (`submit_on_enter(false)`, send on
`InputEvent::PressEnter { secondary: true, .. }`). Enter in the slash list
picks the command (`slash_key` takes "enter" in the composer's `capture_key_down`). Code:
`src/ui/composer.rs`.

The slash / prompt list still floats over the composer's top edge (`absolute().bottom_full()`).

**The thread's width**: the window's whole width from its left edge, as an editor's text — no cap and no centering (the user: centering is not what an editor does; `THREAD_MAX` is gone).

## Lists + editor pages

The only classic settings are the gear's page (providers, look). MCP servers, skills, prompts,
accesses, the Telegram bot and the schedule are a project's environment, edited in its block
(`ui/project_list.rs` `render_env_sections`), not settings tabs.

A list-detail page, where one is needed: left `v_flex().w(px(300.)).border_r_1()` with a scrolling list (`py_2`) and
optional bottom "+ New …" row (`h(bar_h)`, `border_t_1`, muted → foreground on hover); right an
"editor page": 36px toolbar (`pl_4 pr_2 border_b_1`, name/fields left, actions right: ghost
Delete that arms into danger "Confirm delete", primary Save) over a full-height code editor
(Markdown / YAML / JSON with line numbers).

## Chat column

Turn (`text_base`), as in Zed's Delta: **who speaks is an avatar in a left gutter**, no band,
no rules. Two rows, each `h_flex().pl(CHAT_LEFT).pr(CHAT_RIGHT).gap(14).items_start()`: a 24px
round avatar (the person: a dark disc `foreground 78%` with UserRound in `background`; the
agent: a solid `primary` disc with Bot in `primary_foreground` — both solid, because a pale
disc reads smaller than a dark one of the same size; the user noticed), then the text column `flex_1 min_w_0`. Every row (question, answer, composer) pads `py(TURN_Y)` = 14 (centers of a question and its answer 52 apart, as in Delta), so any
two rows are 28 apart whoever speaks — the user compared it with Delta, where they are equal.
The question's text goes through `plain_text` (selectable); the agent's column `gap_3` (summary, steps,
answer, sources, cards). Constants in `ui/mod.rs`: `CHAT_LEFT 24`, `AVATAR 24`,
`CHAT_TEXT = 62`, `CHAT_RIGHT 48`. Everything that lines up with the text uses `CHAT_TEXT` /
`CHAT_RIGHT`: the composer's text (`pl(CHAT_TEXT - 10)`, the textarea's own ~10), its chips, the
waveform, the approval / question cards, the plan strip — measure with `textx.py` that they
start within 0.5pt. One inset, 24px, for the question,
the answer, the composer's text, its chips and the recording waveform (cards like Changed files
end where text does); measure that the three text starts share one x. (It went 16 → 20 → 24 in
small steps on the user's word "a little more air"; Claude Desktop's much larger insets were
the direction, not the target.) Then: one-line summary
("› Done · 13 actions · 1:38 · 1 error", only the error count red, live label as ShimmerText
truncated), foldable steps (left rule, each step unfolds into Arguments/Output boxes in Menlo
xs), Markdown answer split into text / files / diagrams (`references/build-media.md`), quiet notes, hover
actions (time, copy, read aloud, regenerate on last) floating over the answer's top-right corner
(`absolute().top(23px).right_6()`, on the chat background, on the summary line), never a row of
their own under the answer, whose bottom keeps changing.
Column `pb_1` and a real 12px spacer element after trailing cards.

Code blocks in an answer get a header of their own: the language on the left, **Copy** on the
right, the highlighted code under it, one 1px frame around both, **no fill anywhere** (like the
audio, video and Changed files cards). The header is a muted line (`py_1`, `text_base`, the thread's one
size; `render_code`, `ui/media.rs`). Recipe: `ui-content.md`, "Code blocks with a header".

Turn rail instead of a scrollbar: absolutely positioned in the 16px right gutter, vertically
centered, one 8×2px tick per turn (pitch 3–8px), current tick accent, others muted 35%; hover
changes color only (never the size) and shows the question as a tooltip; click scrolls to that
turn (`scroll_to_top_of_item`). Shown only with ≥ 2 turns.
