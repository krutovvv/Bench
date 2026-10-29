# The elements: one crate, one picture

Every screen is built from `bench_ui` (`crates/ui`). It is the only place with pixel values and
direct `gpui-kit` styling: a button, a field, a block, a row — take it from there. Never style
one in place.

**See them:** `cargo run -p storybook` (light), `STORYBOOK_DARK=1 cargo run -p storybook` (dark).
Reference pictures: `crates/storybook/reference/light.png`, `dark.png` (`crates/storybook/shot.sh`
takes them again).

| Need | Call (`bench_ui::…`) | File |
|---|---|---|
| A spacing step | `Gap::{Xs,S,M,L,Xl}` (2/4/8/12/16 px) | `metrics.rs` |
| A height | `FIELD_H` (28, buttons and pickers of a form match it), `BLOCK_HEAD_H` (32), `CHIP_H` (thread chips), `STATUS_H` (29), `bar_h(cx)`, `RING_CLEAR` (air under a focused field) | `metrics.rs` |
| The thread's column, leading and type | `THREAD_MAX_W`, `CHAT_LEFT`, `CHAT_LEFT_NARROW`, `TURN_Y`, `LEAD`, `CODE_LEAD`, `THREAD_META`, `BUBBLE_*`, `COMPOSER_CARD_*`, `SEND_BUTTON` | `metrics.rs` |
| The chrome's type | `SIDEBAR_TEXT` (13), `SIDEBAR_HEAD_TEXT` (11), `BAR_TEXT` (12), `BAR_TITLE` (13) | `metrics.rs` |
| An editor, a picture, a filled control | `editor_line(cx)`, `editor_height(lines, cx)` (as tall as its text), `THUMB`, `BAR_CONTROL_REMS` | `metrics.rs` |
| A row under a group header | `nest_pl(cx)` | `metrics.rs` |
| Text | `body`, `title`, `muted`, `small`, `mono`, `error` | `text.rs` |
| A row, a column, ends apart | `row(Gap::M)`, `stack(Gap::S)`, `spread(Gap::M)`, `spacer()` | `layout.rs` |
| The main action | `primary_button(id, "Save")` | `button.rs` |
| A bordered / quiet / destructive action | `secondary_button`, `ghost_button`, `danger_button` | `button.rs` |
| A compact text button in a block head | `compact_button` | `button.rs` |
| An icon with a tooltip | `icon_button(id, Ico::X, "Tip", on, cx)`; the composer's round ones `round_icon_button` | `button.rs` |
| A form's action in the thread | `form_action(id, "Add", primary, cx)` | `button.rs` |
| Delete without a dialog | `arm_delete_button(id, armed, cx)` | `button.rs` |
| A field | `field(&state)`, `bare_field`, `secret_field` (state from `secret_state`), `search_field` | `field.rs` |
| A labelled field, a form, a setting | `field_row`, `inline_form`, `settings_row`, `settings_heading`, `settings_field` | `form.rs` |
| A Mac segmented control | `segmented(id, labels, chosen, cx, on_pick)`; its parts `segment_track`, `segment` (Light · Dark · System, the terminals' tabs) | `form.rs` |
| Chat \| Code | `mode_switch` | `bars.rs` |
| Files \| Threads (a panel's width) | `mode_switch_full` | `bars.rs` |
| A big choice (the new project's place) | `choice_card` | `form.rs` |
| The sections of a settings panel | `settings_tabs(id, &[(Ico, "Label")], open, cx, on_pick)` — text tabs with icons, the open one underlined in the accent over a rule across the page | `form.rs` |
| A block (insert, card, pane) | `block(title, actions, body, cx)`, `block_frame`, `block_head`, `block_body` | `block.rs` |
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
| Theme | `theme::apply`, `Appearance` | `crates/ui/src/theme.rs` (the choice kept: `src/settings/appearance.rs`) |
| Numbers as text | `plural`, `clock`, `compact`, `clip` | `fmt.rs` |

## Local helpers (one area's, built from the above)

- **Inserts** — `src/inserts/mod.rs`: `icon_button` (an insert head's small icon), `pick_row`,
  `pick_row_removable` (a server or host to pick, trash on hover), `card` (a block an insert
  opened: a file, a task), `file_form` (naming a new file or folder), `row_trash` (a file row's
  two-click delete). Use them inside an insert; never copy them into one.
- **Environment sections** — `src/env/section.rs`: `Item` (a part in the Explorer's tree),
  `add` ("+ Add"), `save`, `revert`, `remove` (two clicks), `form_actions` (a form's Save /
  Revert), `choices` (one of a list), `text` (a value only read), `new_card`, `code` (an editor as
  tall as its text). In `crates/ui`: `settings_heading`, `settings_field`, `segmented`.
- **The Explorer** — `crates/ui/src/ide.rs`: `tree_heading`, `tree_row`, `tree_pl` (a tree's
  indent), `ide_head` (an open document's head), `doc_tab` (its tab), `QUICK_W` (⌘P),
  `LEFT_DOCK`, `RIGHT_DOCK`, `BOTTOM_DOCK` (a dock's size and its range).

A piece that is not here: add it to `crates/ui` (and to the storybook), then use it — not in the
screen that needs it. Ask the person before inventing a new kind of control.

Handlers: `.on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))` on what the
element returns. Ids must be unique: `"name"`, `("name", k)`.
