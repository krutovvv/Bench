# devtools — a Tools mode's logic, ready to copy

Pure Rust, no UI: 19 everyday utilities (Rich Text → Markdown, Markdown preview, passwords, UUID,
Lorem Ipsum, hashes, Base64 text and images, URL, HTML entities, backslash escapes, Unix time with
time zones, JSON ⇄ YAML, string cases, sort & dedupe, cURL → JS/Python/PHP, regex tester, URL
parser, cron parser). Taken from Bench (`~/Projects/bench-kit/src/devtools/`), tested there.

## Use it in an app

1. Copy this folder to `src/devtools/` (without this README) and add `mod devtools;` to `main.rs`.
2. Add to `Cargo.toml` (the template already has serde, serde_json, anyhow):
   ```toml
   regex = "1"
   chrono = "0.4"
   chrono-tz = "0.10"
   croner = "4"
   md-5 = "0.10"
   sha1 = "0.10"
   sha2 = "0.10"
   url = "2"
   percent-encoding = "2"
   rand = "0.8"
   heck = "0.5"
   htmd = "0.5"
   base64 = "0.22"
   serde_yaml = "0.9"
   uuid = { version = "1", features = ["v4"] }
   ```
3. `cargo test devtools` — the tests must pass before any UI.
4. The mode around it: `../tools-mode/app_tools.rs` (state and actions) and
   `../tools-mode/ui_tools.rs` (sidebar, header, panes) are Bench's files as they are — they use
   Bench's `HermesApp`; rename it to the app's struct (`MainApp`) and follow
   `references/build-modes.md` to wire them in. Drop the tools the person doesn't
   need from `GROUPS` in `mod.rs` (and their `Tool` variants).

## Add a tool

1. A variant in `Tool`, its `name`, `hint`, `kind`, `modes`, `sample` in `mod.rs`, and its place in
   `GROUPS`.
2. Its function in the module that fits (`encode`, `convert`, `generate`, `parse`, `markdown`),
   called from `transform` (input → output) or `generate`.
3. A test with real input. Then it shows up in the sidebar by itself.
