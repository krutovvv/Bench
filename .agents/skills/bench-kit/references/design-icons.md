# Icons

**One set for the whole app: Lucide** (https://lucide.dev, ISC licence — keep `assets/icons/LICENSE`).
The user explicitly liked this set; do not mix in other icon families or emoji as UI icons.

## How they are embedded

Each icon is the original Lucide SVG file in `assets/icons/<name>.svg` (24×24 viewBox,
`stroke="currentColor"`, stroke-width 2), compiled into the binary by a macro that generates an
`Ico` enum (`crates/ui/src/icons.rs`):

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

## Icon buttons

`bench_ui::icon_button(id, Ico, tip, on, cx)` (`crates/ui/src/button.rs`): a square, quiet
button with a tooltip for block heads, rows, the header, the sidebar's strip; `on` lights it in
the accent. The composer's plus, mic and Send are `round_icon_button`s of Send's size
(`SEND_BUTTON`) in its field (`src/thread/composer/input.rs`). Plain Lucide everywhere — no
hand-made bold cuts or filled glyphs.

## Sizes

- `xsmall()` (about 12px): sidebar row buttons, a project's "+" and chevron, meta lines, step icons.
- `small()` (about 16px): header toggles, the sidebar's top strip, the status bar, block heads.
- Never larger in chrome. Icon-only buttons always get a tooltip.

## The vocabulary we used

| Meaning | Lucide name |
|---|---|
| stop | square |
| dictate / attach | mic / plus |
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
| git branch / project | git-branch / folder, folder-git-2, layers (no folder), server (a server's Host) |
| changed files / diff | file-diff, file |
| hide / show sidebar | panel-left |
| new window | app-window |
| pin | pin |
| run / pause (cron) | play / pause |
| copy / read aloud | copy / volume-2 |
| open externally | external-link |
| warning / error | circle-alert |
| saved note | save |
| web source | globe |

The app icon (dock) is a separate `assets/logo.svg` rendered to `.icns` at bundle time.
