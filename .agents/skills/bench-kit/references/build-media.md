# Media in a GPUI app

## The rule the user set: lightest thing that works

- **Never a browser engine.** No headless Chrome/Chromium, no WebView, no Puppeteer, no JS
  engines, no npm. The user rejected a headless-Chrome Mermaid/MathJax renderer outright, even
  though it looked better ("какой-то простой, максимально легковесный… чтобы было").
- Pure-Rust crates, macOS system tools (`qlmanage`, `open`) and **ffmpeg** (the user has it and
  uses it) are fine. If a tool is missing, degrade: show the file as a row that opens in its app.
- Diagrams and formulas are "nice to have": simple output is acceptable, heavy machinery is not.

## What each kind becomes

| Kind (by extension) | Shown as | Click |
|---|---|---|
| png jpg gif webp bmp svg | the image, at its pixel size capped to 560×420 pt | opens in Preview |
| mp4 mov m4v webm mkv avi | first frame (Quick Look) with a ▶ badge; plays **in place** via ffmpeg | play / stop; ↗ opens the player |
| mp3 wav ogg m4a aac flac aiff | row: ▶/■ button, name, "playing · 0:12" | plays here (rodio); undecodable → system player |
| md markdown | card: header (name, folder, ↗), rendered Markdown, collapsed to 280 pt + "Show all" | — |
| pdf docx pptx xlsx key pages numbers rtf | first page (Quick Look, 240 wide) + row | opens in its app |
| anything else (glb, stl, zip…) | row: icon, name, folder; the **whole row** is clickable | opens in its default app |
| missing / failed | row with the error in red | click retries |
| ```mermaid block | SVG diagram (mermaid-rs-renderer), theme follows light/dark | opens the SVG |
| `$…$`, `$$…$$` | Unicode text (unicodeit), italic | — |

Code: `media::kind()`, `media::segments()`, `media::fetch()/fetch_now()`, `ui/media.rs`.

## Getting the bytes

- Files may live on another machine (an SSH backend). Read them through the backend's API into
  `state_dir()/media/<hash>/<name>` (keep the file name: `open` and players use it). A file on
  this Mac is used in place — no copy.
- **Lay out at the final size in the first frame** or the view jumps (the user notices every
  jump, especially "jump to the end" on opening a chat). For local files compute everything
  synchronously in render (image size via the `imagesize` crate reads only the header;
  Markdown capped at 256 KB; a cached thumbnail): `fetch_now()`. Only remote downloads and the
  first Quick Look run go to the background; reserve their box meanwhile (a 16:9 frame for a
  video, not a one-line "Loading…").
- Diagrams render in milliseconds: do it in render too, cache the SVG on disk by source + theme.

## Images

```rust
// gpui's img() with only max_w/max_h keeps the intrinsic height → distorted. Give both sizes:
let (w, h) = fit(imagesize::size(&path)?, 560., 420.);   // scale ≤ 1
div().w(w).h(h).border_1().border_color(theme.border).child(img(path).size_full())
```

`img(PathBuf)` decodes off-thread and caches by path; SVG files work too (gpui's SVG renderer
loads the system fonts, so text in SVGs renders).

## Thumbnails: Quick Look

```bash
qlmanage -t -s 720 -o <dir> <file>     # → <dir>/<file name>.png (video frame, PDF/office page)
```

Key the cache folder by path + size + mtime so an overwritten file gets a new picture; reuse the
PNG when it exists (that makes it synchronous next time).

## Audio

`rodio` with `default-features = false, features = ["symphonia-mp3", "symphonia-wav",
"symphonia-flac", "symphonia-vorbis", "symphonia-isomp4", "symphonia-aac"]`. Probe first
(`rodio::Decoder::new(Cursor::new(bytes)).is_ok()`), Opus etc. fail → `cx.open_with_system(path)`.
Play on a thread; poll `is_done()` from the app's 500 ms ticker; one thing plays at a time (stop
speech/video when audio starts and vice versa). Show elapsed time while playing.

## Video in place (ffmpeg)

Find ffmpeg yourself — a Finder-launched app has no Homebrew on PATH:
`/opt/homebrew/bin`, `/usr/local/bin`, `/opt/local/bin`, then `$PATH`. None → open externally.

```text
sound:  ffmpeg -v error -i FILE -vn -ac 2 -ar 44100 -f wav -        → bytes → rodio Playback
frames: ffmpeg -v error -i FILE -an -vf fps=25,scale=W:H -f rawvideo -pix_fmt bgra -
        (W, H even; read exactly W*H*4 bytes per frame on a thread → sync_channel(6))
```

- A frame becomes `Arc<RenderImage>`: `RenderImage::new(vec![image::Frame::new(
  image::ImageBuffer::from_raw(w, h, bgra_bytes)?)])` — gpui wants **BGRA**; add
  `image = { version = "0.25", default-features = false }` (the same crate gpui uses).
  Show it with `img(ImageSource::Render(frame))` in the thumbnail's box.
- Frame i is due at `start + i/25 s`; a 15 ms timer shows the newest due frame, skips late ones.
- **Free every replaced frame from the GPU**: `cx.drop_image(old, None)` from a `cx.spawn` task
  (outside a window update), or `cx.drop_image(old, Some(window))` inside one. Otherwise each
  frame leaks a texture.
- Decode at the box size × the window's scale factor (≤ 2): cheap and sharp.
- Kill the ffmpeg child on drop; stop the video when the chat changes.

## Mermaid without a browser

`mermaid-rs-renderer = { version = "0.3", default-features = false }` (no CLI, no PNG):
`render_with_options(code, RenderOptions { theme: Theme::dark() or modern, .. })` → SVG string
with numeric `width`/`height`. Write it to a cache file, show with `img()` at that size (fit to
the column). Unsupported diagram or syntax error → show the source as a code block.
Split ```mermaid fences out of the Markdown yourself (line scanner that tracks fences); an
unclosed fence (still streaming) stays text.

## LaTeX without a browser

`unicodeit::replace()` turns `x^2`, `\alpha`, `\sum_{i=1}^n`, `\int_0^1`, `\leq`, `\mathbb{R}`
into Unicode. Pre-process what it leaves: `\frac{a}{b}` → `a/b` (parenthesize non-trivial
parts), `\sqrt{x}` → `√x`, `\text{}`/`\mathrm{}`/`\operatorname{}` → their content, drop
`\left`/`\right`/`\displaystyle`, `\,` → space, strip leftover braces (`media::latex_text`).

Hook it into gpui-kit's Markdown (which already parses `$…$`/`$$…$$`) with plugins:

```rust
use gpui_kit::component::text::{MarkdownNode, MarkdownParseContext, MarkdownPlugin, TextView, markdown_ast};
struct InlineMath;
impl MarkdownPlugin for InlineMath {
    fn name(&self) -> &str { "math" }
    fn parse(&self, node: &markdown_ast::Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let markdown_ast::Node::InlineMath(m) = node else { return None };
        // Money is not math: "$5 and $10" — TeX never starts/ends a formula with a space.
        let math = m.value.trim() == m.value && !m.value.is_empty();
        Some(MarkdownNode::new("math", math).text(if math { latex_text(&m.value) } else { format!("${}$", m.value) }))
    }
    fn render(&self, node: &MarkdownNode, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().when(*node.data::<bool>().unwrap_or(&true), |d| d.italic()).child(node.as_text().to_string())
    }
}
// block: is_block() → true, name "math-block", Node::Math, centered
TextView::markdown(id, text).plugin(InlineMath).plugin(BlockMath)
```

## Opening files

`cx.open_with_system(&path)` (default app), `cx.reveal_path(&path)` (Finder). Every media card
has a quiet ↗ "Open" icon; for kinds shown only as a row the whole row opens the file.

## Attachments: paperclip, drag and drop, ⌘V

Reference: `src/app/attachments.rs`, `src/attach.rs`, chips in `src/ui/media.rs`
(`attachment_chip`), composer in `src/ui/composer.rs`.

- **Paperclip**: a round quiet button next to Dictate/Send →
  `cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some("Attach".into()) })`,
  await the receiver in `cx.spawn`, then add the paths.
- **Drop**: on the whole chat column (messages + composer):
  `.drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().list_active)).on_drop(cx.listener(|this, p: &ExternalPaths, _, cx| this.add(p.paths().to_vec(), cx)))`.
  Screenshot thumbnails dragged from the macOS corner preview arrive as files in
  `/var/folders/…` that vanish — copy those before using them.
- **⌘V**: in the composer's `capture_key_down`, on `v` + `modifiers.platform`, read
  `cx.read_from_clipboard()`; `ClipboardEntry::Image(img)` (⌃⇧⌘4 screenshot: `img.format`,
  `img.bytes`) → save to `state_dir()/pasted/…png`; `ClipboardEntry::ExternalPaths` (files copied
  in Finder) → attach; then `cx.stop_propagation()`. Only text → let the field paste it.
- **Chips** over the text field (one wrapping row at the text's inset, `pt_2`): 24px picture
  (`img(path).size(px(24.)).object_fit(ObjectFit::Cover)`) or the kind's icon, the name
  truncated, × to remove. After sending, the same chips (without ×, click opens) sit under the
  user's message. Send is enabled with files and no text.
- Don't restrict types: the agent can handle anything (ffmpeg for video, etc.).

## Splitting content

Hermes answers deliver files as `MEDIA:/abs/path` lines (paths may contain spaces: take the
rest of the line, strip quotes/backticks and trailing punctuation). Also accept a standalone
`![alt](/local/file.png)` line. Inline `…MEDIA:/x.png.` inside prose: replace with `` `x.png` ``
and append the file after the paragraph; accept only absolute paths there. Never parse inside
code fences. Render the segments in order: Markdown text (one `TextView` per text run, with the
math plugins), files, diagrams.
