//! What an answer carries besides text: `MEDIA:<path>` files (the agent's delivery tag, as
//! Hermes defined it), standalone `![](file)` images, ```mermaid diagrams and
//! LaTeX. Files come through the gateway, so a remote (SSH) agent's files show too. Diagrams
//! and formulas are drawn in plain Rust: a diagram as SVG, a formula as Unicode text.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Result, bail};

use crate::connection::{Connection, state_dir};
use crate::model::Target;
use crate::rest;

/// Something fetched or drawn off the UI thread.
#[derive(Clone, Debug)]
pub enum Loaded<T> {
    Pending,
    Ready(T),
    Failed(String),
}

impl<T> From<Result<T>> for Loaded<T> {
    fn from(r: Result<T>) -> Self {
        match r {
            Ok(v) => Loaded::Ready(v),
            Err(e) => Loaded::Failed(format!("{e:#}")),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Image,
    Audio,
    Video,
    Markdown,
    /// PDF and office files: Quick Look draws their first page.
    Doc,
    File,
}

pub fn kind(path: &str) -> Kind {
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" => Kind::Image,
        "mp3" | "wav" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "flac" | "aif" | "aiff" => {
            Kind::Audio
        }
        "mp4" | "mov" | "m4v" | "webm" | "mkv" | "avi" => Kind::Video,
        "md" | "markdown" | "mdown" | "mkd" => Kind::Markdown,
        "pdf" | "doc" | "docx" | "ppt" | "pptx" | "xls" | "xlsx" | "key" | "pages" | "numbers"
        | "rtf" => Kind::Doc,
        _ => Kind::File,
    }
}

pub fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// A piece of an answer, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Text(String),
    /// A file the agent delivered, as it named it.
    Media(String),
    /// A closed ```mermaid block (an unclosed one, still streaming, stays text).
    Mermaid(String),
    /// Any other closed code block: its language (may be empty) and its code. Shown with a
    /// header of its own (the language, a copy button); an unclosed one stays text.
    Code {
        lang: String,
        code: String,
    },
}

/// Split an answer into text, delivered files, diagrams and code blocks.
pub fn segments(answer: &str) -> Vec<Segment> {
    let mut out = vec![];
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<Segment>| {
        if !text.trim().is_empty() {
            out.push(Segment::Text(std::mem::take(text)));
        }
        text.clear();
    };
    // The open code fence: its char and length, and its block (language, body, raw lines).
    let mut fence: Option<(char, usize)> = None;
    let mut block: Option<(String, String, String)> = None;
    for line in answer.split_inclusive('\n') {
        let t = line.trim();
        let lead = line.trim_start();
        let run = |c: char| lead.chars().take_while(|x| *x == c).count();
        if let Some((c, n)) = fence {
            let closes = run(c) >= n && lead.trim_start_matches(c).trim().is_empty();
            match &mut block {
                Some(_) if closes => {
                    let (lang, body, _) = block.take().unwrap();
                    flush(&mut text, &mut out);
                    out.push(if lang.eq_ignore_ascii_case("mermaid") {
                        Segment::Mermaid(body)
                    } else {
                        Segment::Code { lang, code: body }
                    });
                }
                Some((_, body, raw)) => {
                    body.push_str(line);
                    raw.push_str(line);
                }
                None => text.push_str(line),
            }
            if closes {
                fence = None;
            }
            continue;
        }
        if let Some(c) = ['`', '~'].into_iter().find(|c| run(*c) >= 3) {
            let n = run(c);
            fence = Some((c, n));
            // The info string's first word is the language ("```rust ignore" is rust).
            let lang = lead[n..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            block = Some((lang, String::new(), line.to_string()));
            continue;
        }
        if t == "[[audio_as_voice]]" {
            continue;
        }
        let unquoted = t.trim_matches(['`', '"', '\'']);
        if let Some(path) = unquoted.strip_prefix("MEDIA:") {
            let path = clean_path(path);
            if !path.is_empty() {
                flush(&mut text, &mut out);
                out.push(Segment::Media(path));
                continue;
            }
        }
        if let Some(path) = standalone_image(t) {
            flush(&mut text, &mut out);
            out.push(Segment::Media(path));
            continue;
        }
        if line.contains("MEDIA:") {
            let (line, paths) = inline_media(line);
            text.push_str(&line);
            if !paths.is_empty() {
                flush(&mut text, &mut out);
                out.extend(paths.into_iter().map(Segment::Media));
            }
            continue;
        }
        text.push_str(line);
    }
    if let Some((_, _, raw)) = block {
        text.push_str(&raw);
    }
    flush(&mut text, &mut out);
    out
}

fn clean_path(raw: &str) -> String {
    let p = raw.trim().trim_matches(['`', '"', '\'']).trim();
    let p = p.strip_prefix("file://").unwrap_or(p);
    p.trim_end_matches(['.', ',', ';', ':', '!', '?'])
        .to_string()
}

/// `![alt](/some/file.png)` alone on its line, for a local file (web images stay markdown).
fn standalone_image(t: &str) -> Option<String> {
    let inner = t.strip_prefix("![")?.strip_suffix(')')?;
    let (_, url) = inner.split_once("](")?;
    let url = url.split(" \"").next().unwrap_or(url).trim();
    let url = url.trim_start_matches('<').trim_end_matches('>');
    let lower = url.to_lowercase();
    if url.is_empty()
        || ["http:", "https:", "data:"]
            .iter()
            .any(|s| lower.starts_with(s))
    {
        return None;
    }
    Some(clean_path(url))
}

/// "see MEDIA:/tmp/a.png." -> "see `a.png`." and the path; only absolute paths count here,
/// so prose that merely mentions the tag stays as written.
fn inline_media(line: &str) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut paths = vec![];
    let mut rest = line;
    while let Some(at) = rest.find("MEDIA:") {
        let after = &rest[at + 6..];
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        let path = clean_path(&after[..end]);
        if path.starts_with('/') || path.starts_with("~/") {
            let before = rest[..at].trim_end_matches(['`', '"', '\'']);
            out.push_str(before);
            out.push_str(&format!("`{}`", file_name(&path)));
            let tail = &after[..end];
            // Keep the sentence's own punctuation after the name.
            let trimmed = tail.trim_end_matches(['.', ',', ';', ':', '!', '?', '`', '"', '\'']);
            out.push_str(tail[trimmed.len()..].trim_start_matches(['`', '"', '\'']));
            paths.push(path);
        } else {
            out.push_str(&rest[..at + 6 + end]);
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    (out, paths)
}

pub(crate) fn hash_of(parts: &[&str]) -> String {
    let mut h = DefaultHasher::new();
    parts.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// A delivered file, ready to show: where it is on this Mac and what to draw for it.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub local: PathBuf,
    /// An image's size in pixels.
    pub size: Option<(f32, f32)>,
    /// A video's or document's Quick Look picture, with its size.
    pub thumb: Option<(PathBuf, (f32, f32))>,
    /// A Markdown file's text.
    pub text: Option<String>,
}

const MAX_DOWNLOAD: u64 = 512 << 20;

/// The file itself, when the gateway runs on this Mac and it is there.
fn on_this_mac(target: &Target, path: &str) -> Option<PathBuf> {
    let p = match path.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(path),
    };
    (*target == Target::Local && p.is_file()).then_some(p)
}

/// A local file whose preview needs no work (no download, no Quick Look run): ready in the
/// same frame, so the chat lays out at its final height and nothing jumps.
pub fn fetch_now(target: &Target, path: &str) -> Option<Fetched> {
    let local = on_this_mac(target, path)?;
    if matches!(kind(path), Kind::Video | Kind::Doc) && !thumb_file(&local).is_file() {
        return None;
    }
    prepare(local, path).ok()
}

/// Bring a delivered file here (a local gateway's files are read in place) and prepare its
/// preview. Blocking: call off the UI thread.
/// `machine`: the file is on that machine, not the agent's own (a project worked over SSH).
pub fn fetch(
    conn: &Connection,
    target: &Target,
    machine: Option<&str>,
    profile: &str,
    path: &str,
) -> Result<Fetched> {
    let local = match on_this_mac(target, path) {
        Some(p) => p,
        None => download(conn, target, machine, profile, path)?,
    };
    prepare(local, path)
}

/// A local file prepared now (its video frame drawn), so the first paint already has it — for
/// the demo, which has no gateway to fetch through.
pub fn warm(path: &str) {
    let _ = prepare(PathBuf::from(path), path);
}

fn prepare(local: PathBuf, path: &str) -> Result<Fetched> {
    let mut out = Fetched {
        local: local.clone(),
        size: None,
        thumb: None,
        text: None,
    };
    match kind(path) {
        Kind::Image => {
            out.size = imagesize::size(&local)
                .ok()
                .map(|s| (s.width as f32, s.height as f32));
        }
        Kind::Markdown => {
            let mut bytes = vec![];
            std::fs::File::open(&local)?
                .take(256 << 10)
                .read_to_end(&mut bytes)?;
            out.text = Some(String::from_utf8_lossy(&bytes).into_owned());
        }
        Kind::Video | Kind::Doc => out.thumb = quick_look(&local).ok(),
        Kind::Audio | Kind::File => {}
    }
    Ok(out)
}

fn download(
    conn: &Connection,
    target: &Target,
    machine: Option<&str>,
    profile: &str,
    path: &str,
) -> Result<PathBuf> {
    let bytes = rest::download(conn, profile, machine, path, MAX_DOWNLOAD)?;
    let dir = state_dir()
        .join("media")
        .join(hash_of(&[&target.label(), profile, path]));
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(file_name(path));
    std::fs::write(&file, bytes)?;
    Ok(file)
}

/// Where a file's Quick Look picture is kept: by path, size and modification time, so an
/// overwritten file gets a new one.
fn thumb_file(file: &Path) -> PathBuf {
    let meta = std::fs::metadata(file).ok();
    let stamp = meta
        .as_ref()
        .map(|m| {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            format!("{}-{t}", m.len())
        })
        .unwrap_or_default();
    let dir = state_dir()
        .join("media")
        .join("thumbs")
        .join(hash_of(&[&file.to_string_lossy(), &stamp]));
    dir.join(format!(
        "{}.png",
        file.file_name().unwrap_or_default().to_string_lossy()
    ))
}

/// The first frame or page as Quick Look draws it (`qlmanage -t`), with its pixel size.
fn quick_look(file: &Path) -> Result<(PathBuf, (f32, f32))> {
    let png = thumb_file(file);
    if !png.is_file() {
        let dir = png.parent().unwrap_or(Path::new("/tmp"));
        std::fs::create_dir_all(dir)?;
        let status = Command::new("qlmanage")
            .args(["-t", "-s", "720", "-o"])
            .arg(dir)
            .arg(file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() || !png.is_file() {
            bail!("no preview");
        }
    }
    let s = imagesize::size(&png)?;
    Ok((png, (s.width as f32, s.height as f32)))
}

/// A diagram as an SVG file and its size in points. Cached on disk by source and theme.
pub type Diagram = (PathBuf, (f32, f32));

pub fn mermaid_svg(code: &str, dark: bool) -> Result<Diagram> {
    let dir = state_dir().join("diagrams");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!(
        "{}.svg",
        hash_of(&[code, if dark { "dark" } else { "light" }])
    ));
    let svg = match std::fs::read_to_string(&file) {
        Ok(svg) => svg,
        Err(_) => {
            let mut options = mermaid_rs_renderer::RenderOptions::modern();
            if dark {
                options.theme = mermaid_rs_renderer::Theme::dark();
            }
            let svg = mermaid_rs_renderer::render_with_options(code, options)?;
            std::fs::write(&file, &svg)?;
            svg
        }
    };
    let head = &svg[..svg.find('>').unwrap_or(svg.len())];
    let num = |attr: &str| -> Option<f32> {
        let at = head.find(attr)? + attr.len();
        head[at..].split('"').next()?.parse().ok()
    };
    match (num(" width=\""), num(" height=\"")) {
        (Some(w), Some(h)) if w > 0. && h > 0. => Ok((file, (w, h))),
        _ => bail!("diagram has no size"),
    }
}

/// LaTeX as Unicode text: x², α, √x, ∑ᵢ, a/b. Fractions, text and roots are unwrapped here;
/// the symbols are unicodeit's.
pub fn latex_text(tex: &str) -> String {
    fn group(s: &str) -> Option<(String, &str)> {
        let s = s.trim_start();
        let rest = s.strip_prefix('{')?;
        let mut depth = 1;
        for (i, c) in rest.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((rest[..i].to_string(), &rest[i + 1..]));
                    }
                }
                _ => {}
            }
        }
        None
    }
    let wrap = |s: String| {
        if s.chars().count() <= 1 || s.chars().all(|c| c.is_alphanumeric()) {
            s
        } else {
            format!("({s})")
        }
    };
    let mut out = String::new();
    let mut rest = tex;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        let tail = &after[name.len()..];
        match name.as_str() {
            "frac" | "dfrac" | "tfrac" => match group(tail).and_then(|(a, t)| Some((a, group(t)?)))
            {
                Some((a, (b, t))) => {
                    out.push_str(&format!(
                        "{}/{}",
                        wrap(latex_text(&a)),
                        wrap(latex_text(&b))
                    ));
                    rest = t;
                    continue;
                }
                None => out.push_str("\\frac"),
            },
            "sqrt" => match group(tail) {
                Some((a, t)) => {
                    out.push_str(&format!("√{}", wrap(latex_text(&a))));
                    rest = t;
                    continue;
                }
                None => out.push_str("\\sqrt"),
            },
            "text" | "mathrm" | "operatorname" | "textrm" | "mathit" | "textbf" | "mathbf" => {
                match group(tail) {
                    Some((a, t)) => {
                        out.push_str(&a);
                        rest = t;
                        continue;
                    }
                    None => {}
                }
            }
            "left" | "right" | "displaystyle" => {
                rest = tail;
                continue;
            }
            "" => {
                // \, \; \! \  : spacing.
                let c = after.chars().next().unwrap_or(' ');
                if matches!(c, ',' | ';' | ':' | ' ' | '!') {
                    out.push(if c == '!' { '\u{200b}' } else { ' ' });
                    rest = &after[c.len_utf8()..];
                    continue;
                }
                out.push('\\');
                rest = after;
                continue;
            }
            _ => {}
        }
        out.push('\\');
        out.push_str(&name);
        rest = tail;
    }
    out.push_str(rest);
    unicodeit::replace(&out)
        .replace(['{', '}'], "")
        .replace("\u{200b}", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_media_lines_and_diagrams() {
        let a = "Here:\n\nMEDIA:/Users/me/Desktop/Video by x [abc].mp4\n\nNext\n```mermaid\ngraph TD\nA-->B\n```\ntail";
        assert_eq!(
            segments(a),
            vec![
                Segment::Text("Here:\n\n".into()),
                Segment::Media("/Users/me/Desktop/Video by x [abc].mp4".into()),
                Segment::Text("\nNext\n".into()),
                Segment::Mermaid("graph TD\nA-->B\n".into()),
                Segment::Text("tail".into()),
            ]
        );
    }

    #[test]
    fn splits_code_blocks_and_keeps_an_open_one_as_text() {
        let a = "Run:
```bash
ls -la
```
then
```py
print(1)
";
        assert_eq!(
            segments(a),
            vec![
                Segment::Text(
                    "Run:
"
                    .into()
                ),
                Segment::Code {
                    lang: "bash".into(),
                    code: "ls -la
"
                    .into()
                },
                Segment::Text(
                    "then
```py
print(1)
"
                    .into()
                ),
            ]
        );
    }

    #[test]
    fn leaves_code_and_open_diagrams_alone() {
        let a = "```\nMEDIA:/x.png\n```\n~~~mermaid\ngraph TD\n";
        assert_eq!(
            segments(a),
            vec![
                Segment::Code {
                    lang: "".into(),
                    code: "MEDIA:/x.png\n".into()
                },
                Segment::Text("~~~mermaid\ngraph TD\n".into()),
            ]
        );
    }

    #[test]
    fn inline_tags_and_images() {
        let a = "Saved MEDIA:/tmp/a.png. Done\n![shot](/tmp/b.png)\n![web](https://x.io/c.png)\n";
        assert_eq!(
            segments(a),
            vec![
                Segment::Text("Saved `a.png`. Done\n".into()),
                Segment::Media("/tmp/a.png".into()),
                Segment::Media("/tmp/b.png".into()),
                Segment::Text("![web](https://x.io/c.png)\n".into()),
            ]
        );
    }

    #[test]
    fn latex_reads_as_text() {
        assert_eq!(latex_text("E = mc^2"), "E = mc²");
        assert_eq!(latex_text("\\frac{a+b}{2}"), "(a+b)/2");
        assert_eq!(latex_text("\\sqrt{x} \\leq \\alpha"), "√x ≤ α");
        assert_eq!(latex_text("\\text{cost} \\times 3"), "cost × 3");
        assert_eq!(latex_text("\\int_0^1 f(x)\\,dx"), "∫₀¹ f(x) dx");
    }
}
