# Icons

**One set for the whole app: Lucide** (https://lucide.dev, ISC licence — keep `assets/icons/LICENSE`).
The user explicitly liked this set; do not mix in other icon families or emoji as UI icons.

## How they are embedded

Each icon is the original Lucide SVG file in `assets/icons/<name>.svg` (24×24 viewBox,
`stroke="currentColor"`, stroke-width 2), compiled into the binary by a macro that generates an
`Ico` enum (`templates/app/src/icons.rs`):

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

## The "bold cut" for key controls (the composer's circles)

Lucide's thin strokes read as "tiny, not loaded" inside the composer's round buttons. The user
asked for a bold version **in the Font Awesome sense — parts filled, not just thicker lines**,
still Lucide underneath. Three hand-edited copies, same 24×24 grid, named `<name>-bold`/`-fill`:
- `mic-fill.svg`: Lucide's mic with the capsule a filled `rect` (`fill="currentColor"
  stroke="none"`, `x=8.5 y=3 w=7 h=11.5 rx=3.5`), the stand stays a stroke.
- `arrow-up-bold.svg`: Lucide's arrow-up at `stroke-width="2.25"`, the head slightly lower
  (`m6.5 10.5 5.5-5.5 5.5 5.5`).
- `paperclip-bold.svg`: a simpler clip (two loops, no extra curls) at `stroke-width="2.25"`.
- Stop is not an icon: a filled 9–10px square `div` (`rounded(px(1.5))`), never an outlined
  square (that reads as a missing glyph).
Size 13px in a 26px circle, colored like the circle's outline (`muted_foreground` at 75%), not
black. `fill`/`stroke` come from the SVG itself (currentColor), so the `icons!` macro needs
nothing special. Use this cut only where the user asked for it; everything else stays Lucide.

## Sizes

- `xsmall()` (about 12px): status bar, sidebar row buttons, meta lines, step icons.
- `small()` (about 16px): header toggles, mode toggles, editor toolbars.
- The composer's round buttons: 13px icons in 26px circles (`.size(px(13.))`, see below).
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
