# Crates: what to use for which job

All of these compile together with gpui-kit 0.8 in Bench (`~/Projects/bench/Cargo.toml`) —
take these versions. Many are already in gpui-kit's dependency tree, so adding them costs no extra
build time: check with `grep -A1 '^name = "<crate>"' Cargo.lock` first.

## Already proven in Bench

| Job | Crate (version) | Use |
|---|---|---|
| UI | `gpui-kit = "0.8.0"` (+ tree-sitter features) | everything visual; see SKILL.md |
| JSON / YAML | `serde`, `serde_json = "1"`, `serde_yaml = "0.9"` | settings, APIs, JSON ⇄ YAML |
| HTTP | `ureq = { version = "2.12", features = ["json"] }` | blocking calls on a background thread |
| WebSocket | `tungstenite = "0.30"` | the agent gateway's JSON-RPC |
| Async channels | `async-channel = "2"` | thread → UI events |
| Errors | `anyhow = "1"` | `Result<T>` in I/O code |
| Terminal | `portable-pty = "0.9"` + `alacritty_terminal = "0.26"` | a PTY and a VT parser (Bench `src/inserts/terminal/session.rs`) |
| Text diff | `similar = "2"` | changed files' +/- (Bench `src/diff.rs`) |
| IDs | `uuid = { version = "1", features = ["v4"] }` | `Uuid::new_v4()` |
| Base64 | `base64 = "0.22"` | `STANDARD.encode(bytes)` / `.decode(s)` |
| Images | `imagesize = "0.13"` (size without decoding), `image = { version = "0.25", default-features = false }` | previews, sizes (Bench decodes nothing itself: gpui's `img()` does) |
| Image processing (crop, resize, convert) | `image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp"] }` | **with codecs** — without them `image::open` fails. Recipe below; checked with gpui-kit 0.8 |
| macOS notifications | `objc2-user-notifications = { version = "0.3", default-features = false, features = ["std", "block2", "UNNotificationContent", "UNNotificationRequest", "UNNotificationSound", "UNNotificationTrigger", "UNUserNotificationCenter"] }` | "the agent finished" (Bench `src/notify.rs`); only inside the .app |
| SQL highlighting | gpui-kit feature `tree-sitter-sql` (grammar `tree-sitter-sequel`) | needs `cc` 1.2.x: keep Bench's `Cargo.lock`; after a `cargo update`, run `cargo update -p cc --precise 1.2.67` |
| Audio | `cpal = "0.15"` (mic), `hound = "3.5"` (WAV), `rodio = "0.20"` (playback) | dictation, TTS playback |
| Diagrams | `mermaid-rs-renderer = { version = "0.3", default-features = false }` | Mermaid → SVG, no browser |
| Math | `unicodeit = "0.2"` | LaTeX → Unicode in text |
| Regex | `regex = "1"` | `RegexBuilder::new(p).case_insensitive(..).build()` |
| Dates | `chrono = "0.4"` + `chrono-tz = "0.10"` | `Utc.timestamp_opt`, `t.with_timezone(&"Asia/Tokyo".parse::<chrono_tz::Tz>()?)` |
| Cron | `croner = "4"` | `Cron::from_str(expr)?`, `.describe()` (English), `.iter_after(Local::now()).take(5)` |
| Hashes | `md-5 = "0.10"`, `sha1 = "0.10"`, `sha2 = "0.10"` | `sha2::Sha256::digest(bytes)` (one `digest` 0.10 for all three) |
| URLs | `url = "2"`, `percent-encoding = "2"` | parse, query pairs; RFC 3986 encoding |
| Random | `rand = "0.8"` | `rand::rngs::OsRng` for passwords |
| String cases | `heck = "0.5"` | `ToSnakeCase`, `ToLowerCamelCase`, … |
| HTML → Markdown | `htmd = "0.5"` | tables included; `HtmlToMarkdown::builder().options(..).skip_tags(vec!["script","style"]).build().convert(html)`; it pads table columns to the longest cell — compact the rows after (`crates/devtools/src/markdown.rs`) |

Bench's `crates/devtools/src/` uses most of the last block, with tests — read it for working
calls; those crates are its own dependencies (`crates/devtools/Cargo.toml`), not the app's.

## Recipe: crop to a size (e.g. a 1200×630 cover)

A tool as a pure function with a test (put it in `crates/devtools/src/`):
```rust
use image::imageops::FilterType;

/// The largest centered box with the target's aspect, resized to exactly w×h.
pub fn cover(src: &std::path::Path, dst: &std::path::Path, w: u32, h: u32) -> anyhow::Result<()> {
    let img = image::open(src)?;
    let (sw, sh) = (img.width(), img.height());
    let (cw, ch) = if sw as u64 * h as u64 > sh as u64 * w as u64 {
        ((sh as u64 * w as u64 / h as u64) as u32, sh)
    } else {
        (sw, (sw as u64 * h as u64 / w as u64) as u32)
    };
    img.crop_imm((sw - cw) / 2, (sh - ch) / 2, cw, ch)
        .resize_exact(w, h, FilterType::Lanczos3)
        .save(dst)?;            // the format follows dst's extension
    Ok(())
}
#[test]
fn crops_to_1200x630() {
    let d = std::env::temp_dir();
    let (a, b) = (d.join("in.png"), d.join("out.jpg"));
    image::RgbImage::new(2000, 1500).save(&a).unwrap();
    cover(&a, &b, 1200, 630).unwrap();
    assert_eq!(image::open(&b).unwrap().dimensions(), (1200, 630));
}
```
(`use image::GenericImageView as _;` for `.dimensions()`.) Its UI is a utility in the `/tools` insert
(`src/inserts/tools/`): drop or paste an image in, the cropped preview out, Save as…
(`build-insert.md`).

## macOS without crates

Some jobs need no crate — the system already does them (rung 3 of the engineering rules):

- **The clipboard's HTML / RTF** (rich text copied from a browser, Word, Notes):
  `osascript -e 'the clipboard as «class HTML»'` prints `«data HTML3C68…»` (hex); for RTF use
  `«class RTF »` and convert with `/usr/bin/textutil -stdin -format rtf -convert html -stdout`.
  (gpui's clipboard gives plain text only.) Code: `crates/devtools/src/markdown.rs`.
- Open a file or URL with its app: `cx.open_with_system(path)` / `/usr/bin/open`.
- File pickers: `cx.prompt_for_paths(..)`, `cx.prompt_for_new_path(dir, Some(name))`.
- SSH: `/usr/bin/ssh` with the user's config and a ControlMaster socket (Bench `src/ssh.rs`).

## Finding a crate that isn't here

1. Search [lib.rs](https://lib.rs) or crates.io by the job ("html to markdown", "cron parser");
   [blessed.rs](https://blessed.rs) lists the community's standard choices.
2. `cargo info <crate>`: version, license, repository. Prefer: maintained (a release this year),
   widely used, pure Rust (no system libraries to install), MIT/Apache.
3. Check its features and README for what you need (e.g. does it do tables?) before adding it.
4. Add it, `cargo build`, and write a small test with real input before building UI on it.
5. If it pulls a second version of a big crate (html5ever, image, tokio), look for an
   alternative or its feature flags first.
