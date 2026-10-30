//! What an answer carries besides text: `MEDIA:<path>` files (the agent's delivery tag), standalone `![](file)` images, ```mermaid diagrams and
//! LaTeX. Files come through the agent, so a remote (SSH) agent's files show too. Diagrams
//! and formulas are drawn in plain Rust: a diagram as SVG, a formula as Unicode text.

pub(crate) mod state;
pub(crate) mod attach;
pub(crate) mod audio;
pub(crate) mod pauses;
pub(crate) mod chip;
mod diagram;
mod fetch;
mod latex;
mod playback;
pub(crate) mod text;
pub(crate) mod video;
pub(crate) mod view;
mod voice;

pub use diagram::*;
pub use fetch::*;
pub use latex::*;

use std::path::Path;

use anyhow::Result;

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
}
