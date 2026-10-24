# Layout recipes

All snippets use `use gpui_kit::*; use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex}; use gpui_kit::prelude::FluentBuilder as _;`.

## Window skeleton

```
┌ sidebar 280 ─────┬ header 36 ────────────────────────────┬ right panel 640 (optional) ┐
│ (lights)   🔧 ☰  │ title …          [chip] [▢] [±] [⊞]   │ tabs 36                    │
│ search band 36   ├───────────────────────────────────────┤ viewer | list (resizable)  │
│──────────────────│ content (chat / settings page)        │                            │
│ GROUP · n        │                                       │                            │
│ rows…            │                                       │                            │
│──────────────────├───────────────────────────────────────┤                            │
│ [+ New session]  │ composer (36)                         │                            │
├──────────────────┴───────────────────────────────────────┴────────────────────────────┤
│ status bar 28: target · profile ……… speed · context · approvals · effort · model · ⟳ ⚙ │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

```rust
v_flex().size_full().child(
    div().flex_1().min_h_0().child(
        h_resizable("layout")
            .child(resizable_panel().size(px(280.)).size_range(px(200.)..px(480.)).child(sidebar))
            .child(resizable_panel().child(main))
            .when(show_right, |d| d.child(
                resizable_panel().size(px(640.)).size_range(px(360.)..px(1400.)).child(right))),
    ),
).child(statusbar)
```

The title bar is transparent (`appears_transparent: true`): the sidebar starts with a 35px
strip beside the traffic lights (the mode toggles on its right); the main header (36px) sits at
the top of the main column.

## Facing bars (the alignment users notice)

```rust
/// The header's 36 less the one rule a bar draws: 35 at the 14px font.
pub fn bar_h(cx: &App) -> Pixels { cx.theme().font_size * 2.5 }
```

All bars are the header's height, and every rule crossing the window is one line:
- Sidebar top strip `h(px(35.))` + the search band's top rule = the header's 36 → the band's
  top rule *is* the header's bottom rule, on one pixel row.
- Search band: `h(bar_h + 1)`, rules above and below, background color, no frame.
- Sidebar footer: `h(bar_h + 1)` with the primary button `.flex_1().h_full()`, no rule of its
  own. Composer: `border_t_1` + a row `min_h(bar_h)` → its top rule lies exactly on the
  button's top edge. (A footer of `bar_h` against a composer of `bar_h + rule` was a visible
  1px step; the user spotted it.)
- A strip under the header that faces the search band (Tasks' tag strip): `min_h(bar_h)`, so
  its bottom rule meets the band's.
- Any "+ New item" row at the bottom of a list: `h(bar_h + 1)`.
Measure it (`build-verification.md`): the rules' pixel rows must be equal, not "about".

## Sidebar rows and groups

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
(`flex_1 min_w_0 text_sm font_weight(MEDIUM) truncate`), then context chips (e.g. project ·
branch as a ghost xsmall Button with a dropdown), then 24px icon toggles (`p_1`, muted, active =
accent + `list_active` bg, tooltip with the shortcut, e.g. "New window  ⌘⇧N"). Window-level
toggles sit at the right end: "Open files" (shown while the chat or server has files to open)
then **the SSH mode switch last** (it changes the whole window: a pressed/unpressed terminal
icon). New window lives in the status bar before Settings; Tools and Tasks over the sidebar.
Item-specific actions (open in Zed, files) go to the item's "⋯" menu, not the header. In settings mode the same strip holds tabs: each tab
`h_full px_4 text_sm border_r_1`, active = content bg + foreground, others muted.

## Status bar

28px, `bg(status_bar) border_t_1 text_xs`. Left: where (target), who (profile). Right: live
readouts and pickers. A picker is a ghost xsmall Button **with the icon inside it** (so hover
highlights both) and a `dropdown_menu` of checked items:

```rust
Button::new(id).ghost().xsmall().icon(Ico::Cpu.icon()).label(label).dropdown_menu(move |menu, _, _| {
    items.iter().fold(menu, |m, (text, checked, f)| m.item(PopupMenuItem::new(text.clone()).checked(*checked).on_click(…)))
})
```

Readouts with more detail open a `Popover` on click (stats table, speed chart) — see
`references/ui-gotchas.md`. Hide a picker until its data is loaded (no empty dropdowns, no
default values pretending to be real: keep a `ready` flag, reset it on reconnect / profile
switch). All picker buttons `.text_color(muted)` so the bar is one tone.

Row menus: separate unrelated groups with `.separator()` (Pin | colors | Open in new window |
Delete).

## Composer

```rust
v_flex().id("composer").relative().w_full()
    .bg(theme.background).border_t_1().border_color(theme.border)    // the row inside: min_h(bar_h)
    .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {   // ↑ ↓ Tab Esc for the popup
        if !ev.keystroke.modifiers.modified() && this.popup_key(ev.keystroke.key.as_str(), window, cx) {
            cx.stop_propagation();
        }
    }))
    .children(popup)                                       // absolute, see below
    .child(h_flex().w_full().min_h(bar_h(cx)).items_center()
        // No vertical padding: the textarea's own keeps one line inside the 35 (more lines grow it).
        .child(div().flex_1().min_w_0().pl(px(11.))   // + the textarea's own ~13 → text at the chat's 24
            .child(Textarea::new(&self.composer).appearance(false)))
        .child(h_flex().flex_none().gap_2().pl_2().pr_4().items_center()
            .child(clip).child(mic).child(send)))
```

The buttons are **26px circles** with 13px bold-cut icons (`rounded_full`) — the only round
things in the UI, since they float inside the field: quiet one = 1px border + muted icon (hover
`list_hover`), filled one = accent bg + `primary_foreground` icon (Send when there is text; Stop
turn = foreground, ⌘.; Stop recording = danger with a 9px ■, Esc cancels; while recording,
Send stays and means "stop, transcribe, send"). While recording, the text area is
replaced by a waveform 26 high (bars `flex_1`, 2px gap, height `2 + level × 20`, accent with
opacity) with equal gaps: `pl_6` (24) from the divider and `pr_4` + the button column's `pl_2` =
24 to the button. Everything inside is sized to the bar, never the bar to its contents.

Popup list attached to the composer (slash commands, `@` prompts):

```rust
div().absolute().bottom_full().left_0().right_0().px_4()          // chat width, 16px sides
    .child(div().bg(theme.popover).border_t_1().border_l_1().border_r_1().border_color(theme.border)
        .child(v_flex().id("list").max_h(px(28. * 8.)).overflow_y_scroll().track_scroll(&scroll).children(rows)))
```

No bottom border (the composer's top rule is it), no gap. Rows 28px: name in Menlo xs, one-line
description muted, category right-aligned faint. Show only while the input is a bare `/word` (or
the last word is `@word`); a `/` elsewhere or an unknown word (a path) is plain text.

## Lists + editor pages (settings tabs)

Settings tab order the user wants: MCP, Skills, Prompts, Config, Cron, Gateways, Profiles,
Settings (Appearance + misc as one scrolling page).

List-detail tab: left `v_flex().w(px(300.)).border_r_1()` with a scrolling list (`py_2`) and
optional bottom "+ New …" row (`h(bar_h)`, `border_t_1`, muted → foreground on hover); right an
"editor page": 36px toolbar (`pl_4 pr_2 border_b_1`, name/fields left, actions right: ghost
Delete that arms into danger "Confirm delete", primary Save) over a full-height code editor
(Markdown / YAML / JSON with line numbers).

## Chat column

Turn (`text_sm`) = the person's message as a **band from edge to edge** (`bg secondary`,
`border_t_1` except the first turn, `border_b_1`, `px_6 py_4`; text through `plain_text` so it
can be selected and copied) — the session reads as strips, one per question — then the agent's
side in `v_flex().px_6().py_6().gap_3()` — 24px all round. One inset, 24px, for the question,
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
audio, video and Changed files cards). The header is as tall as Changed files' line (`py_1`,
`text_xs`). Recipe: `ui-content.md`, "Code blocks with a header".

Turn rail instead of a scrollbar: absolutely positioned in the 16px right gutter, vertically
centered, one 8×2px tick per turn (pitch 3–8px), current tick accent, others muted 35%; hover
changes color only (never the size) and shows the question as a tooltip; click scrolls to that
turn (`scroll_to_top_of_item`). Shown only with ≥ 2 turns.

## Right panel (files)

Tabs (Changes / Files) 36px; body = `h_resizable` of viewer and a 240px file list (160–600),
list hidden by default and remembered. Viewer toolbar: file name (tooltip = path), Diff /
Before / After toggles (1px bordered chips, active = accent border), Save, Open in Zed. Diff
uses gpui-kit `Diff` with the whole file as context (collapsed to 3 lines, expandable).
