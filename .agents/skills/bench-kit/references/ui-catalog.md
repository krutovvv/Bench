# The elements: one crate, one picture

Every screen is built from `bench_ui` (`crates/ui`). It is the only place with pixel values and
direct `gpui-kit` styling: a button, a field, a block, a row — take it from there. Never style
one in place.

**See them:** `cargo run -p storybook` (light), `STORYBOOK_DARK=1 cargo run -p storybook` (dark).
Reference pictures: `crates/storybook/reference/light.png`, `dark.png` (`crates/storybook/shot.sh`
takes them again).

| Need | Call (`bench_ui::…`) | File |
|---|---|---|
| Any size, spacing step | `FIELD_H`, `BLOCK_HEAD_H`, `CHAT_*`, `TURN_Y`, `Gap::{Xs,S,M,L,Xl}` (2/4/8/12/16 px), `bar_h`, `editor_height` | `metrics.rs` |
| Text | `body`, `title`, `muted`, `small`, `mono`, `error` | `text.rs` |
| A row, a column, ends apart | `row(Gap::M)`, `stack(Gap::S)`, `spread(Gap::M)`, `spacer()` | `layout.rs` |
| The main action | `primary_button(id, "Save")` | `button.rs` |
| A bordered / quiet / destructive action | `secondary_button`, `ghost_button`, `danger_button` | `button.rs` |
| A compact text button in a block head | `compact_button` | `button.rs` |
| An icon with a tooltip | `icon_button(id, Ico::X, "Tip", on, cx)` | `button.rs` |
| A form's action in the thread | `form_action(id, "Add", primary, cx)` | `button.rs` |
| Delete without a dialog | `arm_delete_button(id, armed, cx)` | `button.rs` |
| A field | `field(&state)`, `bare_field`, `secret_field` (state from `secret_state`), `search_field` | `field.rs` |
| A labelled field, a form, a setting | `field_row`, `inline_form`, `settings_row`, `segmented` | `form.rs` |
| A block (insert, card, pane) | `block(title, actions, body, cx)`, `block_head`, `block_body` | `block.rs` |
| Who speaks | `avatar(agent, theme)` | `block.rs` |
| One line saying nothing is there | `empty_state` | `block.rs` |
| A list row, a group | `list_row`, `pick_row`, `group_chevron`, `nest_guide`, `head_icon` | `list.rs` |
| A file's icon | `file_icon(name, dir, theme)` | `list.rs` |
| Bars | `header_bar`, `pane_bar`, `status_text`, `status_button`, `search_band` | `bars.rs` |
| Icons | `Ico::…` (Lucide, `assets/icons/`) | `icons.rs` |
| Theme | `theme::apply`, `Appearance` | `theme.rs` |
| Numbers as text | `plural`, `clock`, `compact`, `clip` | `fmt.rs` |

A piece that is not here: add it to `crates/ui` (and to the storybook), then use it — not in the
screen that needs it. Ask the person before inventing a new kind of control.

Handlers: `.on_click(cx.listener(|this, _, window, cx| this.save(window, cx)))` on what the
element returns. Ids must be unique: `"name"`, `("name", k)`.
