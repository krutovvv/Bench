# A new Bench, the house way

Every app this kit makes is **the same app underneath**: Rust, GPUI via gpui-kit 0.8, Bench's
design one to one, Bench's icon. Don't propose another stack, theme or layout; don't ask about
them. There are two starts — pick by what the person asked for:

| They asked for… | Start | What it is |
|---|---|---|
| Their workbench, their work, anything with an agent, "help me with X" — **the default** | `scripts/new-bench.sh <crate>` | A copy of the reference Bench: the Hermes chat, settings, providers, SSH + terminals, mini IDE, Tools, Tasks. Then switch off what they don't use (`bench-modes-remove.md`) and add their modes (`build-modes.md`). |
| Explicitly an app **without** an agent ("just a text editor", "a viewer") | `scripts/new-app.sh <crate> "<Title>"` | The frame alone: sidebar, header, status bar, settings, theme, `kit.rs`. The chat can be added later as a panel (`build-hermes-client.md`). |

Never put an agent into an app the person asked to have without one; never build a Bench
without the agent when they didn't ask for that.

**Later they want the agent in that app** ("an icon top right opens a chat about the open
document; the agent edits it, I see the changes"): don't port the client into the plain app —
it is thousands of lines wired through Bench's state. Make it a Bench: `new-bench.sh`, switch
off SSH, Tools and Tasks (`bench-modes-remove.md`), keep the **IDE**. That is exactly the asked
shape: the chat with the open file beside it (⌘B, ⌘P to open a file); "Add to chat" in the
editor attaches the file, the agent edits it, and the editor follows the file on disk while it
has no unsaved edits of its own (Bench `src/app/ide.rs`, the `changed on disk` branch). Move
the plain app's own pieces over as a mode (`build-modes.md`). Tell the person this is the
route and why.

## 1. Create the project

```bash
~/Projects/bench-kit/.agents/skills/bench-kit/scripts/new-bench.sh <crate-name>            # the usual start
~/Projects/bench-kit/.agents/skills/bench-kit/scripts/new-app.sh <crate-name> "<Title>"   # no agent asked for
# e.g. new-bench.sh anna-bench      new-app.sh clip-notes "Clip Notes"
```

- `crate-name`: lowercase-with-dashes; it is also the binary, `~/Library/Application Support/<crate>`
  and the bundle id `local.<crate>` (stable — macOS privacy grants follow it).
- The icon is Bench's for every app (both scripts copy `~/Projects/bench-kit/assets/logo.svg`); a
  Bench keeps the name "Bench" in the window and the Dock.
- It lands in `~/Projects/<crate>` with `git init` done. Nothing is overwritten (it refuses an
  existing folder).
- Builds go to one shared folder for all such apps (`~/Library/Caches/gpui-apps/target`, set in
  the project's `.cargo/config.toml`; `./target` links to it, so `./target/debug/<crate>` works).
  gpui compiles once: the first app's first build takes minutes, the next apps' seconds.
- `new-bench.sh` copies the reference Bench's committed files (`git ls-files` of
  `~/Projects/bench-kit`, the kit included), so a new Bench starts from its latest commit.

## 2. What `new-app.sh`'s template already has

```
src/main.rs        app entry, open_main_window() (transparent title bar, 1240×800)
src/state.rs       state_dir(), log_error() (Finder apps have no stderr)
src/theme.rs       One Light / One Dark palettes, accents, S/M/L sizes, Appearance load/save
src/icons.rs       Lucide icons embedded (Ico::X.icon(), Ico::Loader.live(true) spins)
src/app.rs         MainApp: all state + actions; Item list persisted to items.json, search,
                   Panel::Main / Panel::Settings, set_appearance(), follows macOS dark mode
src/ui/mod.rs      layout: resizable sidebar 280 | header 36 + body; status bar 28
src/ui/sidebar.rs  top strip for mode toggles (35), search band (36), rows (accent stripe,
                   hover-only actions), "+ New" filling the bottom bar (36, rules line up)
src/ui/statusbar.rs one muted tone, settings gear on the right
src/ui/settings.rs Appearance: Theme Light/Dark/System, Accent, Size (segmented, square)
src/ui/kit.rs      ready house-style blocks: primary/icon/status buttons, list_row, group_header,
                   tab_strip, header_bar, sidebar_top, mode_toggle, search_band, bottom_bar,
                   status_text, settings_row, segmented, empty_state, arm_delete_button
                   (references/ui-catalog.md)
examples/          (in this skill, not copied) gallery.rs + gallery.png: a screen using every
                   kit block — copy calls from it
scripts/bundle-macos.sh  release build → dist/<Title>.app, icon from assets/logo.svg, Info.plist,
                   codesign with the stable identifier
assets/icons, assets/themes, assets/logo.svg (generated), .gitignore
```

`templates/app` has Bench's exact gpui-kit features (all 35 grammars, SQL included) and Bench's
`Cargo.lock`, so builds reuse the same compiled crates, the grammars match Bench's `lang.rs`,
and `cc` stays at 1.2.67 (the SQL grammar needs it). Don't `cargo update` the whole lock.

## 3. Turn it into the app — step by step

Do these one at a time. After EACH step: `cargo build`, fix every error, run the app.

1. **Build and run the untouched template** (`cargo build && ./target/debug/<crate>`). If this
   fails, fix it before anything else — your own code is not the problem yet.
2. **Decide which ready components you need**: look up every part of the request in
   `references/ui-content.md` and write the list down for yourself.
   Everything that is in that table is copied, not written. Everything that is not there: ask
   the user whether it is needed for the first version.
3. **Replace the body** (`render_body` in `src/ui/mod.rs`) with the main component. Build, run.
   Rows, buttons, tabs, headers and settings come from `src/ui/kit.rs` — never style your own.
4. **Replace `Item`** in `src/app.rs` with the app's own data (or remove the list if the app has
   none, keeping the sidebar's search and bottom button for the app's main action). Build, run.
5. **Status bar**: put the app's one or two useful facts there (e.g. "Ln 12, Col 4"). Build, run.
6. **Check it**: run with a sample input (a file path on the command line works for file apps),
   take a screenshot of your own window (`references/build-verification.md`).
7. Then §4: bundle, open, report.

Keep the frame as it is — sidebar, header, status bar, settings, theme, fonts, spacing. The
user wants the apps to look the same as Bench.

**Example of the right size.** "A simple code viewer like the mini IDE: open a file, line
numbers, highlighting" = this template + recipes 1–5 of `references/ui-content.md`, finished in
`templates/examples/text-viewer/` (copy its five files; ~150 changed lines, no new crates,
zero warnings). The template's item list becomes Recent files, the header shows the file name. If your plan for a request like this has a
custom text buffer, your own highlighter or new crates, the plan is wrong.

More:
- Need more icons: copy from `~/Projects/bench-kit/assets/icons/` or download from Lucide
  into `assets/icons/`, one line in the `icons!` list.
- Media in the content (images, video, audio, Markdown/Mermaid/LaTeX, attachments, drag and
  drop): `references/build-media.md`.
- Protected resources (microphone, LAN over SSH, camera): add their usage strings to the
  Info.plist in `scripts/bundle-macos.sh` (the template leaves them commented out).

## 4. Build, check, ship

```bash
cargo build && ./target/debug/<crate>            # dev run (foreground, timeout 600)
scripts/bundle-macos.sh                         # dist/<Title>.app — it runs the release build itself
open "dist/<Title>.app"
```

`bundle-macos.sh` builds release on its own: don't run `cargo build --release` before it (that
doubles a ~5 min first build). Bundle before the first "done" report, without being asked; leave
the app in `dist/` (copy to /Applications only when the user asks).

Then follow `references/build-workflow.md`: zero warnings, `cargo fmt`, verify by measuring your
own window, commit, `ship.sh` (a new app per commit, opened next to the running one), report
honestly what was checked.
