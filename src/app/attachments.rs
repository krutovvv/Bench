//! The composer's files — the paperclip, drag and drop onto the chat, ⌘V of a screenshot — and
//! its references to the project's code sent from the Explorer (⌘L, a tab dropped on the thread).

use std::path::PathBuf;

use gpui_kit::*;

use super::BenchApp;
use crate::attach;

/// A piece of the project's code sent to the thread: a file, or lines of it with their text. The
/// message carries it under its words (`message`), as the agent reads it and the thread shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeRef {
    /// As the agent reaches it (on the project's machine).
    pub path: String,
    /// 1-based, both included.
    pub lines: Option<(usize, usize)>,
    pub text: Option<String>,
}

impl CodeRef {
    /// What its chip says: `landing.tsx:12–20`.
    pub fn label(&self) -> String {
        let name = self.path.rsplit('/').next().unwrap_or(&self.path);
        match self.lines {
            Some((a, b)) if a == b => format!("{name}:{a}"),
            Some((a, b)) => format!("{name}:{a}–{b}"),
            None => name.to_string(),
        }
    }

    /// Its part of the message: the path (and lines), then the code fenced in its language.
    pub fn message(&self) -> String {
        let at = match self.lines {
            Some((a, b)) if a == b => format!("{}:{a}", self.path),
            Some((a, b)) => format!("{}:{a}-{b}", self.path),
            None => self.path.clone(),
        };
        match &self.text {
            Some(t) => {
                let lang = self.path.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
                format!("`{at}`\n```{lang}\n{}\n```", t.trim_end())
            }
            None => format!("`{at}`"),
        }
    }
}

/// The 1-based lines a selection (byte offsets into `text`) covers; a selection ending at a
/// line's start does not take that line.
pub fn lines_of(text: &str, range: std::ops::Range<usize>) -> (usize, usize) {
    let clamp = |at: usize| {
        let mut at = at.min(text.len());
        while !text.is_char_boundary(at) {
            at -= 1;
        }
        at
    };
    let (start, mut end) = (clamp(range.start), clamp(range.end));
    if end > start && text[..end].ends_with('\n') {
        end -= 1;
    }
    let line = |at: usize| text[..at].matches('\n').count() + 1;
    (line(start), line(end.max(start)))
}

/// The message's words with the references under them.
pub fn with_references(text: String, refs: &[CodeRef]) -> String {
    std::iter::once(text)
        .filter(|t| !t.is_empty())
        .chain(refs.iter().map(CodeRef::message))
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl BenchApp {
    /// The open thread's references waiting in its composer.
    pub fn thread_references(&self) -> Vec<CodeRef> {
        self.references
            .get(&self.thread_key())
            .cloned()
            .unwrap_or_default()
    }

    /// A reference to the project's code into the open thread's composer (once).
    pub fn add_reference(&mut self, r: CodeRef, cx: &mut Context<Self>) {
        let list = self.references.entry(self.thread_key()).or_default();
        if !list.contains(&r) {
            list.push(r);
        }
        cx.notify();
    }

    pub fn remove_reference(&mut self, k: usize, cx: &mut Context<Self>) {
        if let Some(list) = self.references.get_mut(&self.thread_key()) {
            if k < list.len() {
                list.remove(k);
            }
        }
        cx.notify();
    }

    pub fn add_attachments(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for p in paths {
            if p.is_file() && !self.attachments.contains(&p) {
                self.attachments.push(p);
            }
        }
        cx.notify();
    }

    /// The paperclip: files picked in the system's dialog, attached as a drop would.
    pub fn pick_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                let _ = this.update(cx, |this, cx| this.add_attachments(paths, cx));
            }
        })
        .detach();
    }

    pub fn remove_attachment(&mut self, k: usize, cx: &mut Context<Self>) {
        if k < self.attachments.len() {
            self.attachments.remove(k);
            cx.notify();
        }
    }

    /// ⌘V with an image (a screenshot copied with ⌃⇧⌘4) or files copied in Finder attaches them;
    /// returns false when the clipboard holds only text (the text field pastes it).
    pub fn paste_attachments(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(item) = cx.read_from_clipboard() else {
            return false;
        };
        let mut paths = vec![];
        for entry in item.entries() {
            match entry {
                ClipboardEntry::ExternalPaths(p) => paths.extend(p.paths().iter().cloned()),
                ClipboardEntry::Image(img) => {
                    let ext = match img.format {
                        ImageFormat::Jpeg => "jpg",
                        ImageFormat::Gif => "gif",
                        ImageFormat::Webp => "webp",
                        ImageFormat::Svg => "svg",
                        ImageFormat::Bmp => "bmp",
                        ImageFormat::Tiff => "tiff",
                        _ => "png",
                    };
                    if let Ok(p) = attach::save_pasted(&img.bytes, ext) {
                        paths.push(p);
                    }
                }
                _ => {}
            }
        }
        if paths.is_empty() {
            return false;
        }
        self.add_attachments(paths, cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{CodeRef, lines_of, with_references};

    #[test]
    fn a_selection_names_its_lines() {
        let text = "one\ntwo\nthree\nfour\n";
        // "two\nthree"
        assert_eq!(lines_of(text, 4..13), (2, 3));
        // whole lines, ending after the newline: the next line is not taken
        assert_eq!(lines_of(text, 4..14), (2, 3));
        // a caret
        assert_eq!(lines_of(text, 0..0), (1, 1));
        // past the end, and inside a letter of two bytes
        assert_eq!(lines_of("é\nb", 1..99), (1, 2));
    }

    #[test]
    fn a_reference_reads_as_path_lines_and_code() {
        let r = CodeRef {
            path: "/p/src/a.rs".into(),
            lines: Some((3, 5)),
            text: Some("fn a() {}\n".into()),
        };
        assert_eq!(r.label(), "a.rs:3–5");
        assert_eq!(r.message(), "`/p/src/a.rs:3-5`\n```rs\nfn a() {}\n```");
        let f = CodeRef {
            path: "/p/README".into(),
            lines: None,
            text: None,
        };
        assert_eq!(f.label(), "README");
        assert_eq!(with_references("look".into(), &[f]), "look\n\n`/p/README`");
    }
}
