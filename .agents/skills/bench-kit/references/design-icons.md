# Icons

**One set for the whole app: Lucide** (https://lucide.dev, ISC licence — keep `assets/icons/LICENSE`).
The user explicitly liked this set; do not mix in other icon families or emoji as UI icons.

## How they are embedded

Each icon is the original Lucide SVG file in `assets/icons/<name>.svg` (24×24 viewBox,
`stroke="currentColor"`, stroke-width 2), compiled into the binary by a macro that generates an
`Ico` enum (Bench's `src/icons.rs`):

```rust
icons! {
    ArrowUp => "arrow-up",
    Loader => "loader-circle",
    // …
}
// use:
Ico::Search.icon().xsmall().text_color(theme.muted_foreground)
Ico::Loader.live(true)   // spinning variant for loaders (see references/ui-gotchas.md)
```

Adding an icon = download the SVG from lucide.dev into `assets/icons/`, add one line to the
macro, keep the list alphabetical. Icons take color from `text_color` (currentColor).

## The composer's buttons (history)

For a while the composer had round 26px buttons with a hand-made "bold cut" of Lucide (a filled
mic capsule, thicker arrow and clip). The user found them noise; since 2026-10-24 the composer
uses the same small quiet buttons as an answer's actions (`chat::icon_button`, plain Lucide,
hover only — `design-layout.md`, Composer). Only `mic-fill.svg` remains (the mic glyph); don't
bring back circles or bold cuts unless asked.

## Sizes

- `xsmall()` (about 12px): status bar, sidebar row buttons, meta lines, step icons.
- `small()` (about 16px): header toggles, the sidebar's top strip, editor toolbars.
- The composer's buttons: `xsmall()`, as an answer's actions (`chat::icon_button`).
- Never larger in chrome. Icon-only buttons always get a tooltip.

## The vocabulary we used

| Meaning | Lucide name |
|---|---|
| send / stop | arrow-up / square |
| dictate | mic |
| loading (must spin) | loader-circle |
| new item | plus |
| row menu | ellipsis |
| delete | trash |
| close | x |
| search | search |
| settings | settings |
| restart / reload | refresh-cw / rotate-ccw |
| server / target | server |
| profile / user | user-round |
| reasoning effort | brain |
| model | cpu |
| context / memory | memory-stick |
| speed | gauge |
| approvals | shield |
| subagent | bot |
| git branch / project | git-branch / folder, folder-open, folder-tree |
| changed files / diff | file-diff, file |
| file list panel | panel-right |
| new window | app-window |
| pin | pin |
| run / pause (cron) | play / pause |
| copy / read aloud | copy / volume-2 |
| open externally | external-link |
| warning / error | circle-alert |
| saved note | save |
| web source | globe |

The app icon (dock) is a separate `assets/logo.svg` rendered to `.icns` at bundle time.
