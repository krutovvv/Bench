# The elements: one crate, one picture

Every screen is built from `bench_ui` (`crates/ui`). It is the only place with pixel values and
direct `gpui-kit` styling: a button, a field, a block, a row — take it from there. Never style
one in place.

**See them:** `cargo run -p storybook` (light), `STORYBOOK_DARK=1 cargo run -p storybook` (dark).
Reference pictures: `crates/storybook/reference/light.png`, `dark.png` (`crates/storybook/shot.sh`
takes them again).

| Need | Call (`bench_ui::…`) | File |
|---|---|---|
| A spacing step | `Gap::{Zero, Hair, Xxs, Xs, S, M, Ml, L, Xl, Xxl, Huge}`, in rems: `.gap(Gap::M)`, `.px(Gap::L)`. A screen writes no bare Tailwind spacing (`gap_2`, `px_3`) and no literal pixel size | `metrics.rs` |
| A height | `FIELD_H` (28, buttons and pickers of a form match it), `BLOCK_HEAD_H` (32), `CHIP_H` (thread chips), `STATUS_H` (29), `bar_h(cx)`, `RING_CLEAR` (air under a focused field) | `metrics.rs` |
| The thread's column, leading and type | `THREAD_MAX_W`, `CHAT_LEFT`, `CHAT_LEFT_NARROW`, `TURN_Y`, `LEAD`, `CODE_LEAD`, `THREAD_META`, `BUBBLE_*`, `COMPOSER_CARD_*`, `COMPOSER_MAX_LINES`, `SEND_BUTTON` | `metrics.rs` |
| The chrome's type | `SIDEBAR_TEXT` (13), `SIDEBAR_HEAD_TEXT` (11), `BAR_TEXT` (12), `BAR_TITLE` (13) | `metrics.rs` |
| An editor, a picture, a filled control | `editor_line(cx)`, `editor_height(lines, cx)` (as tall as its text), `THUMB`, `BAR_CONTROL_REMS` | `metrics.rs` |
| A row under a group header | `nest_pl(cx)` | `metrics.rs` |
| Text | `body`, `title`, `muted`, `small`, `mono`, `error` | `text.rs` |
| A row, a column, ends apart | `bench_ui::row(Gap::M)`, `stack(Gap::S)`, `spread(Gap::M)`, `spacer()` | `layout.rs` |
| A clickable element of your own | `own_press(div())`: the press is its own, the thread under it does not take it. Every clickable element below does this already | `button.rs` |
| The main action | `primary_button(id, "Save")` | `button.rs` |
| A bordered / quiet / destructive action | `secondary_button`, `ghost_button`, `danger_button` | `button.rs` |
| A compact text button in a block head | `compact_button` | `button.rs` |
| An icon with a tooltip | `icon_button(id, Ico::X, "Tip", on, cx)`; the composer's round ones `round_icon_button` | `button.rs` |
| A form's action in the thread | `form_action(id, "Add", primary, cx)` | `button.rs` |
| Delete without a dialog | `arm_delete_button(id, armed, cx)`, `confirm_buttons` | `button.rs` |
| A small bordered chip that does something | `chip(id, cx)` (a preset, a page's host, a pane strip's text button), `chip_tight` (less room at its sides); its text size is the caller's | `button.rs` |
| A field | `field(&state)`, `bare_field`, `secret_field` (state from `secret_state`), `search_field`; `flush_field()` a slot that sets a bare field's text where the words around it start | `field.rs` |
| A labelled field, a form, a setting | `field_row`, `inline_form`, `settings_row`, `settings_heading`, `settings_field` | `form.rs` |
| One of several things a settings page keeps | `settings_card(title, role, rows, check, actions, cx)` (a provider, an evaluator server: its role word, its rows, its last check, its actions), `settings_add(id, label, cx)` the link under them ("Add a provider…") | `form.rs` |
| A Mac segmented control | `segmented(id, labels, chosen, cx, on_pick)`, with `on_pick(cx, handler)` as `cx.listener` for a pick; its parts `segment_track`, `segment` (Light · Dark · System, the terminals' tabs). In a pane's strip, where a track is too tall: `segmented_cells` | `form.rs` |
| Chat \| Code | `mode_switch` | `bars.rs` |
| A big choice (the new project's place) | `choice_card` | `form.rs` |
| The sections of a settings panel | `settings_tabs(id, &[(Ico, "Label")], open, cx, on_pick)` — text tabs with icons, the open one underlined in the accent over a rule across the page | `form.rs` |
| A block (insert, card, pane) | `block(title, actions, body, cx)`, `block_frame`, `block_head`, `block_body` | `block.rs` |
| A fold | `fold_head`; "Show all / Show less": `fold_words(open, shut)` its words, `fold_more` the line, `fold_foot` as a block's foot, `fold_row` as a row of a block's list | `block.rs` |
| A block of code (an answer's, a step card's) | `code_frame(title, action, body, folded, more, cx)`; `code_note` a command shown as it will run inside a card; `editor_frame(cx)` a code editor among a form's fields | `block.rs` |
| A part of a popover | `popover_section(ruled, cx)`, `popover_title(title, cx)` | `block.rs` |
| A turn of the thread (chat look) | `turn_row_plain`; the document look's `turn_row` | `block.rs` |
| Who speaks (the document look only) | `avatar(agent, theme)` | `block.rs` |
| One line saying nothing is there | `empty_state` | `block.rs` |
| A list row, a group | `list_row(id, title, meta, selected, nested, actions, cx)`, `pick_row(id, icon, label, detail, cx)`, `group_chevron`, `nest_guide` | `list.rs` |
| A choice of the agent's question (wraps) | `choice_row(id, cx)` — a row as wide as the card, never a `Button` (its label is one line) | `list.rs` |
| A head's icon, its "+" room | `head_icon`, `head_icon_in(icon, color, cx)` (a marked project's colour), `head_plus_slot()` | `list.rs` |
| A file's icon | `file_icon(name, dir, theme)` | `list.rs` |
| Bars | `header_bar`, `pane_bar`, `status_text`, `status_button`, `search_band` (the sidebar's filled search field) | `bars.rs` |
| A state dot, a mark | `dot(color)`, `tag_dot(color)`, `mark_color(i)` (Finder's tags), `faint(theme)` (the quietest text) | `list.rs`, `mark.rs`, `text.rs` |
| Icons | `Ico::…` (Lucide, `assets/icons/`) | `crates/ui/src/icons.rs` |
| Theme | `theme::apply`, `Appearance` | `crates/ui/src/theme/mod.rs`, `theme/palettes.rs` (the choice kept: `src/settings/appearance.rs`) |
| Numbers as text | `plural`, `clock`, `compact`, `clip` | `fmt.rs` |

## Local helpers (one area's, built from the above)

- **Inserts** — `src/inserts/mod.rs`: `scroll_pane` (a pane that scrolls inside the thread with its
  own wheel), `Inset` (how a kind's body sits in its block, `InsertKind::inset`). Use them inside an
  insert; never copy them into one.
- **Environment sections** — `src/env/section.rs`: `PartRow` (a part in the Explorer's tree),
  `add` ("+ Add"), `save`, `revert`, `remove` (two clicks), `form_actions` (a form's Save /
  Revert), `choices` (one of a list), `text` (a value only read), `new_card`, `code` (an editor as
  tall as its text). In `crates/ui`: `settings_heading`, `settings_field`, `segmented`.
- **The gear** — `src/settings/server_form.rs`: `ServerForm`, what a provider's and an evaluator
  server's cards share.
- **The Explorer** — `crates/ui/src/ide.rs`: `tree_heading`, `tree_row`, `tree_pl` (a tree's
  indent), `ide_head` (an open document's head), `doc_tab` (its tab), `QUICK_W` (⌘P),
  `LEFT_DOCK`, `RIGHT_DOCK`, `BOTTOM_DOCK` (a dock's size and its range).

A piece that is not here: add it to `crates/ui` (and to the storybook), then use it — not in the
screen that needs it. Ask the person before inventing a new kind of control.

Every new element goes on the storybook's page too (`crates/storybook`).

Handlers: `.on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))` on what the
element returns. Ids must be unique: `"name"`, `("name", k)`.
