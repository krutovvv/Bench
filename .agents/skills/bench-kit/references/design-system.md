# Design system (Zed style)

Reference: the Zed editor's look — flat, dense, quiet chrome, content first, slightly rounded.
Working copy of everything below: `crates/ui/src/theme/` (`mod.rs`: `apply`; `palettes.rs`: the
`Scheme`s — VS Code (Dark+/Light+) is the first, so it is the default),
`crates/ui/src/metrics.rs` (every size), and the default syntax pair
`assets/themes/vs-light.json`, `vs-dark.json` (written by `scripts/gen_themes.py`).

## Base switches (`theme.rs`)

```rust
Theme::update(cx, |t| {
    t.radius = px(4.);          // buttons, fields, menus
    t.radius_lg = px(6.);       // blocks use rounded_md
    t.shadow = false;           // no drop shadows (popovers keep only the OS shadow)
    t.font_size = px(14.);      // UI font = rem: every rems()/px_4()/h_8() scales with it
    t.mono_font_size = px(12.); // code, editors, diffs, line numbers
});
```

Apply order matters: `Theme::change(mode, None, cx)` (reloads the registered palette for
light/dark) **then** `Theme::update(cx, |t| …)` with your overrides. `Theme::global_mut` edits do
not propagate to already-built windows; `Theme::update` does.

## Light palette (default)

| Token | Hex | Use |
|---|---|---|
| background | `#FAFAFA` | content (chat, editors) |
| foreground | `#242529` | text |
| sidebar | `#EAEAEC` | left panel (no rule against the thread) |
| sidebar_accent / list_hover | `#DEDEE1` / `#E6E6E8` | hover in the sidebar / elsewhere |
| title_bar, status_bar, tab_bar | `#E4E4E6` | chrome a step darker than content |
| border, sidebar_border | `#D9D9DA` | all 1px rules |
| input, window_border | `#C9C9CB` | input outlines |
| muted / muted_foreground | `#EBEBEC` / `#6E6F75` | secondary text, meta lines |
| secondary | `#EDEDEE` | quiet fills (secondary buttons) |
| primary (accent) | `#4F6FD8` | selection stripe, primary button, focus, live values |
| danger / success / warning | `#C9503F` / `#4F8A3F` / `#B7791F` | errors only / ok / caution |
| list_active | accent at 14% | selected row background |
| selection | accent at 25% | text selection |
| scrollbar thumb | `#242529` at 18% (hover 30%) | — |

Dark (One Dark): content `#282C33`, panels `#2F343E`, chrome `#3B414D`, borders `#464B57`,
text `#DCE0E5`, muted `#A9AFBC`; accent lightened (blue `#74A5F2`), primary_foreground dark
`#1E2127`. Full list: `crates/ui/src/theme.rs` → `dark_palette`.

Accent presets (light, dark): Blue `#4F6FD8/#74A5F2`, Violet `#7A5BD0/#A98BF0`, Green
`#3F8A50/#86C08A`, Teal `#16848A/#5FC0C4`, Amber `#B7700F/#E2B05A`, Coral `#CF5A3C/#EE8C6C`,
Rose `#C2457A/#EC86AE`. Hover = lightness −5% (light) / +6% (dark).

Item colors (user-assigned labels, e.g. session colors): `#D0605A #D99A4E #D6C24A #6DB86A
#5C8FD8 #9A6AD6 #D46A9F` (saved by index — append new ones at the end) — shown as a 2px left stripe plus the same color at 12% as row background
(22% when selected). A chosen color wins over the accent stripe — on hover too: hover keeps
the row's color, slightly denser (20% / 30%), never the generic grey ("color epilepsy").

## Syntax highlighting

One Light / One Dark as gpui-kit `HighlightTheme` JSON (Zed-compatible keys: `editor.background`,
`editor.line_number`, `syntax.keyword.color`, …), loaded with
`serde_json::from_str::<HighlightTheme>(include_str!(…))` and set as `t.highlight_theme`.
One theme for every editor and diff in the app (config YAML, MCP JSON, skills Markdown, files).
Key colors (light): keyword `#A626A4`, function `#4078F2`, string `#50A14F`, number/constant
`#986801`, type `#C18401`, property/tag `#E45649`, operator `#0184BC`, comment `#A0A1A7` italic.

## Typography

- UI: system font, 14px base. The thread is **one size**, `text_base` — messages, answers,
  steps, cards' heads, the composer; secondary things are muted, never smaller. Chrome (sidebar
  titles at `SESSION_TITLE` 14, header, status bar, block heads) is `text_sm`. Medium weight
  only for titles and the header.
- Code: Menlo, `mono_font_size` 12 — editors, diffs, command names, cron schedules.
- Size presets (⌘+ / ⌘−; no setting): Small 13/11, Normal 14/12, Large 15/13. Rem-based
  spacing scales with the font; fixed sizes don't — rems for what must keep proportions.
- Leading: the thread's is `LEAD` (1.5rem), shared by messages and the composer; wrapped list
  titles `line_height(relative(1.25))`.

## Sizes and gaps (`crates/ui/src/metrics.rs`)

Never a `px(…)` in a screen: name one of these, or add it there first.

| Name | Value | What |
|---|---|---|
| `Gap::Xs/S/M/L/Xl` | 2 / 4 / 8 / 12 / 16 | every gap between elements; `Xl` the inset from a panel's edge |
| `FIELD_H` | 28 | a field; buttons, pickers, chips of a form match it |
| `BLOCK_HEAD_H` | 32 | the head of every block (`block_head`) |
| `CHIP_H` | = `FIELD_H` | attachment, queued message, a plan's head |
| `RING_CLEAR` | 4 | air under a focused field (the ring is drawn outside) |
| `STATUS_H` | 29 | the status bar with its rule |
| `bar_h(cx)` | font × 2.5 (35) | every bar without its rule; + 1 = the header's 36 |
| `TURN_Y` | 14 | every thread row pads top and bottom by it: rows 28 apart |
| `CHAT_LEFT` | 28 (2 × `TURN_Y`) | the avatar's x |
| `AVATAR` | 24 | the avatar disc |
| `CHAT_TEXT` | 66 (`CHAT_LEFT + AVATAR + 14`) | where the thread's text starts |
| `CHAT_RIGHT` | = `CHAT_TEXT` | as much free on the right |
| `LEAD`, `THUMB`, `BAR_CONTROL_REMS` | 1.5 / 64 / 2 | leading; a picture's square; a filled control outside a bar |
| `nest_pl`, `editor_line`, `editor_height(lines)` | fns | a row under a group header; an editor as tall as its text |

Other fixed sizes: header 36, sidebar 280 (resizable 200–480), popovers 260–300 wide.

## Color usage rules

- Muted text for everything secondary (meta, counters, timestamps, hints).
- Accent only for "active/selected/live/primary". Never decorative. Primary buttons in the accent
  (`button_primary = primary`).
- Red only for errors (error counts, failed steps, destructive confirm, the fallback model).
- Live states: calm accent + motion (spinning loader, shimmer).
- Status bar: every label and icon `muted_foreground`; only live speed (accent) and context
  thresholds (amber/red) differ. Context > 60% amber, > 85% red.
- Corners: `rounded_md` on every bordered block (`bench_ui::block`), the theme radius on
  controls; `rounded_full` only for avatars and overlay badges (a ▶ on a video frame).
