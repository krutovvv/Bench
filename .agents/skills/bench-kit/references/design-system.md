# Design system (a Mac app's)

Reference: Apple's own apps — Xcode, Finder, Mail, Messages, System Settings. Working copy of
everything below: `crates/ui/src/theme/` (`mod.rs`: `apply`; `palettes.rs`: `BENCH_LIGHT`,
`BENCH_DARK`), `crates/ui/src/metrics.rs` (every size), `crates/ui/src/mark.rs` (Finder's tag
colours), and the syntax pair `assets/themes/bench-light.json`, `bench-dark.json` (written by
`scripts/gen_themes.py`, checked against the palette by `cargo test -p bench_ui`).

## Base switches (`theme/mod.rs` `apply`)

```rust
Theme::update(cx, |t| {
    t.radius = px(4.);          // the library's controls
    t.radius_lg = px(6.);       // rounded_md
    t.shadow = false;           // shadows are set where macOS has them, not by the library
    t.font_size = px(14.);      // the rem: the thread's words
    t.mono_font_size = px(12.); // code, editors, diffs, the terminal
});
```

Apply order matters: `Theme::change(mode, None, cx)` then `Theme::update(cx, |t| …)`. ⌘+ / ⌘−
step the whole size (13/11, 14/12, 15/13); there is no size setting.

## Palette

| Token | Light | Dark | Use |
|---|---|---|---|
| background | `#FFFFFF` | `#1A1B1F` | pages: the thread, editors, settings |
| foreground | `#1B1C1F` | `#E4E5E8` | text |
| sidebar, chrome (title bar, status bar, tabs) | `#F6F6F7` | `#202126` | the light grey frame |
| border | `#E2E3E7` | `#303238` | every hairline |
| sidebar_accent | `#E6E7EB` | `#30333A` | a selected row's ground |
| list_hover | `#EDEDF0` | `#2A2C31` | a hovered row |
| muted_foreground | `#5D6068` | `#A0A3AB` | secondary text |
| `faint(theme)` | muted at lower alpha | — | the quietest: dirs, notes, the agent's thoughts |
| popover | `#FFFFFF` | `#25272C` | popovers, the completion list |
| primary (accent) | `#2F6BED` | `#5B9BFF` | the one accent |
| danger / success / warning | `#C93C3C` / `#1E8A4C` / `#B7791F` | | errors / ok / waiting |

Marks are Finder's tag colours (`bench_ui::mark_color`), drawn as a `tag_dot`.

## Typography (`metrics.rs`)

| Name | Size | Where |
|---|---|---|
| rem (`text_base`) | 14 | the thread's words: messages, answers, the composer |
| `THREAD_META` | 13 | the work line, steps, block heads, cards' text in the thread |
| `SIDEBAR_TEXT` | 13 | sidebar and tree rows, controls (segmented, form actions, completions, popovers) |
| `SIDEBAR_HEAD_TEXT` | 11 semibold, muted | group heads (Host, a project, Agent, Project), popover section titles, log lines |
| `BAR_TEXT` | 12 | the status bar, chips in bars, an IDE head |
| `BAR_TITLE` | 13 semibold | the window's title, the Explorer's project pull-down |
| mono | 12 | code (`CODE_LEAD` 1.618, Zed's), the terminal, diffs |

Leading: the thread's prose `LEAD` (1.5rem), shared with the composer.

## Sizes (`crates/ui/src/metrics.rs`)

| Name | Value | What |
|---|---|---|
| `TITLE_BAR_H` | 40 | the title bar across the window |
| `STATUS_H` | 29 | the status bar with its rule |
| `bar_h(cx)` | font × 2.5 | the Explorer's bar, the documents' tabs, IDE heads |
| `FIELD_H` | 28 | a field; form actions match it |
| `SEGMENT_H` / `MODE_SEGMENT_H` | 24 / 22 | a segmented control's segment / Chat · Code's |
| `LIST_ROW_H` | 26 | a list row: steps, changed files |
| `COMPLETION_ROW_H`, `COMPLETION_GAP` | 26, 6 | the `/` list's rows, its air over the field |
| `BLOCK_HEAD_H` | 32 | every block's head (`block_head`) |
| `SIDEBAR_ROW_INSET`, `SIDEBAR_TEXT_X` | 10, 20 | a row's inset from the sidebar's edge, where its text starts |
| `THREAD_ROW_PY`, `GROUP_GAP` | 4, 20 | a sidebar row's padding, the air between groups |
| `TAG_DOT`, `DOT` | 8, 6 | a mark, a state dot |
| `THREAD_MAX_W` | 860 | the thread's column, centred |
| `CHAT_LEFT`, `CHAT_LEFT_NARROW`, `THREAD_NARROW_W` | 28, 16, 640 | the thread's sides; narrower than 640 (a dock) takes 16 |
| `BUBBLE_PX/PY/R` | 14 / 8 / 16 | the person's message |
| `COMPOSER_CARD_R/PX/PY/PB` | 14 / 8 / 8 / 12 | the composer's field and the air under it |
| `SEND_BUTTON`, `STOP_MARK` | 26, 9 | Send (and the plus and mic beside it), Stop's square |
| `STEP_VERB_W` | 88 | a step's verb column |
| `SETTINGS_W`, `PAGE_W` | 760, 680 | a settings page's column, the new project's page |
| `TURN_Y` | 14 | a thread row's top and bottom |
| `Gap::Xs…Xl` | 2 / 4 / 8 / 12 / 16 | gaps |
| `editor_height(lines)` | fn | an editor as tall as its text |

`CHAT_TEXT`, `AVATAR`, `CHAT_RIGHT`, `COMPOSER_TOOLS_*` belong to the document look
(`src/thread/look.rs`), kept in code, not shown.

## Colour use

- Muted for everything secondary; `faint` for what the eye should pass over.
- The accent only for live, chosen or primary — never decoration.
- Red only for errors (an error count, a failed step, a destructive confirm, the fallback).
- Waiting for the person: amber (`warning`), in the dot and the line over the composer.
- Live states: the accent plus motion (a spinner, the title's shimmer).
