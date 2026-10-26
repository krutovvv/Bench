# Design system (Zed style)

Reference: the Zed editor's look — flat, square, dense, quiet chrome, content first.
Working copy of everything below: Bench's `crates/ui/src/theme.rs`, `assets/themes/one-light.json`,
`assets/themes/one-dark.json`.

## Base switches

```rust
Theme::update(cx, |t| {
    t.radius = px(0.);          // no rounded corners anywhere
    t.radius_lg = px(0.);
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
| sidebar | `#EFEFF0` | left panel, lists in settings |
| sidebar_accent / list_hover | `#E2E2E4` / `#E6E6E8` | hover, selected row |
| title_bar, status_bar, tab_bar | `#E9E9EA` | chrome one step darker than content |
| border, sidebar_border | `#D9D9DA` | all 1px rules |
| input, window_border | `#C9C9CB` | input outlines |
| muted / muted_foreground | `#EBEBEC` / `#6E6F75` | secondary text, meta lines |
| secondary | `#EDEDEE` | user-message blocks, code boxes |
| primary (accent) | `#4F6FD8` | selection stripe, primary button, focus, live values |
| danger / success / warning | `#C9503F` / `#4F8A3F` / `#B7791F` | errors only / ok / caution |
| list_active | accent at 14% | selected row background |
| selection | accent at 25% | text selection |
| scrollbar thumb | `#242529` at 18% (hover 30%) | — |

Dark (One Dark): content `#282C33`, panels `#2F343E`, chrome `#3B414D`, borders `#464B57`,
text `#DCE0E5`, muted `#A9AFBC`; accent lightened (blue `#74A5F2`), primary_foreground dark
`#1E2127`. Full list in Bench's `crates/ui/src/theme.rs` → `dark_palette`.

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

- UI: system font, 14px base. `text_sm` for body/lists (≈12.25) — including chat answers and the
  user's messages —, `text_xs` for meta lines, status bar, labels (≈10.5). Medium weight only for
  titles and the header.
- Code: Menlo (`font_family("Menlo")`), `mono_font_size` 12 — editors, diffs, command names,
  cron schedules, version strings.
- Size presets (⌘+ / ⌘−; no longer in Settings): Small 13/11, Normal 14/12, Large 15/13. Because the UI
  font is the rem, rem-based spacing scales with it; fixed `px()` sizes don't — use rems for
  anything that must keep proportions (bars, controls).
- Line height: default GPUI ≈1.6 is too loose for wrapped titles → `line_height(relative(1.25))`
  for list titles; body text keeps the default.

## Spacing scale

GPUI helpers are rem-based: `p_1` = 0.25rem (≈4px), `p_2` ≈8, `p_3` ≈12, `p_4` ≈16 (at 16px
rem; at 14px rem they are ~12% smaller — that is fine, keep using the scale consistently).

| Thing | Value |
|---|---|
| Horizontal inset of content from any panel edge | 16 (`px_4`) in the sidebar and lists; 24 (`px_6`) in the main reading area (chat, task list, task card) |
| List row | `pl(14) + border_l_2` (text at 16), `pr_3`, `py_2` |
| Group header in a sidebar | icon of the group's kind + name `text_sm` medium muted, chevron and "+" on the right; no counts, no rules (`design-layout.md`) |
| Section header in a list (Tasks: "October", "This Evening") | `flex_none px_6 pt_4 pb_1 text_xs` muted, a rule under it |
| Every bar (search band, bottom button, composer, tag strip) | the header's 36: `bar_h = font_size × 2.5` (35) + its 1px rule |
| Header strip | 36px, `pl_4 pr_2`, title `text_sm` medium, truncate |
| Status bar | 28px, `text_xs`, items `px_2`, pickers with icon inside the button |
| Chat turn | question band `px_6 py_4` edge to edge; the agent's side `px_6 py_6 gap_3`; `text_sm`; the composer's text at the same 24 |
| Composer buttons | 26px circles, 13px bold-cut icons, `gap_2`; quiet = 1px border, filled = accent (Send with text) / foreground (Stop turn) / danger (Stop recording) |
| Cards under an answer (Changed files, code block, audio, video) | 1px frame, no fill; header line `px_2 py_1 text_xs` muted (code: `pl_4`, language left, Copy right) |
| Settings list column | 300px wide, right border |
| Sidebar | 280px (resizable 200–480); no right panel |
| Popover panels | 260–300px wide, rows `px_3 py_0p5 text_xs`, key muted left / value right |

## Color usage rules

- Muted text for everything secondary (meta, counters, timestamps, hints).
- Accent only for "active/selected/live/primary". Never decorative.
- Red only for errors (error counts, failed steps, destructive confirm button).
- Live states: calm accent + motion (spinning loader, shimmer). Recording shows a round filled
  Stop button (■) — the one place a solid danger-colored control is fine.
- Status bar: every label and icon `muted_foreground` (pickers too: `Button…text_color(muted)`);
  only live speed (accent) and context thresholds (amber/red) differ.
- Radius stays 0 everywhere; circles (`rounded_full`) only for the floating composer buttons and
  overlay badges (a ▶ on a video frame).
- Status thresholds: context > 60% amber, > 85% red.
