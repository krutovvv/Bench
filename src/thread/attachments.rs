//! The composer's files — the paperclip, drag and drop onto the chat, ⌘V of a screenshot — and
//! its references sent from the Explorer: the project's code (⌘L, a tab dropped on the thread),
//! its language servers' problems.

use std::path::PathBuf;

use gpui_kit::*;

use super::ThreadView;
use crate::app::BenchApp;
use crate::media::attach;

/// What the Explorer sends to the thread: a chip in the composer, then part of the message under
/// its words (`message`), as the agent reads it and the thread shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Reference {
    Code(CodeRef),
    Problems(ProblemsRef),
    Terminal(TerminalRef),
}

impl Reference {
    /// What its chip says.
    pub fn label(&self) -> String {
        match self {
            Reference::Code(c) => c.label(),
            Reference::Problems(p) => p.label(),
            Reference::Terminal(t) => t.label(),
        }
    }

    /// Its part of the message.
    pub fn message(&self) -> String {
        match self {
            Reference::Code(c) => c.message(),
            Reference::Problems(p) => p.message(),
            Reference::Terminal(t) => t.message(),
        }
    }

    /// Its chip's icon: the file's, or the problems'.
    pub fn icon(&self, theme: &gpui_kit::component::Theme) -> (bench_ui::icons::Ico, Hsla) {
        match self {
            Reference::Code(c) => bench_ui::file_icon(&c.path, false, theme),
            Reference::Problems(_) => (bench_ui::icons::Ico::CircleAlert, theme.danger),
            Reference::Terminal(_) => (bench_ui::icons::Ico::Terminal, theme.muted_foreground),
        }
    }
}

/// A language server's errors and warnings sent to the thread: of one file, or all the project's
/// servers know (`explorer/problems.rs`), a line each.
#[derive(Clone, Debug, PartialEq)]
pub struct ProblemsRef {
    /// The file they are of; None: the project's.
    pub of: Option<String>,
    /// `path:line:col error: message (source)`.
    pub lines: Vec<String>,
}

impl ProblemsRef {
    /// `Problems · landing.tsx (3)`.
    pub fn label(&self) -> String {
        let n = self.lines.len();
        match &self.of {
            Some(p) => format!("Problems · {} ({n})", p.rsplit('/').next().unwrap_or(p)),
            None => format!("Problems ({n})"),
        }
    }

    pub fn message(&self) -> String {
        let of = self
            .of
            .as_ref()
            .map_or(String::new(), |p| format!(" in `{p}`"));
        format!(
            "Problems from the language servers{of}:\n```\n{}\n```",
            self.lines.join("\n")
        )
    }
}

/// A terminal's output sent to the thread (the bottom dock's, `inserts/terminal/panel.rs`): what
/// was selected in it, or its last lines.
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalRef {
    /// Where it runs: a server's name, or "this Mac".
    pub machine: String,
    pub text: String,
}

impl TerminalRef {
    /// `Terminal · dev (12 lines)`.
    pub fn label(&self) -> String {
        let n = self.text.lines().count();
        format!(
            "Terminal · {} ({n} line{})",
            self.machine,
            if n == 1 { "" } else { "s" }
        )
    }

    pub fn message(&self) -> String {
        format!(
            "The terminal on {} shows:\n```\n{}\n```",
            self.machine,
            self.text.trim_end()
        )
    }
}

/// A piece of the project's code sent to the thread: a file, or lines of it with their text.
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
pub fn with_references(text: String, refs: &[Reference]) -> String {
    std::iter::once(text)
        .filter(|t| !t.is_empty())
        .chain(refs.iter().map(Reference::message))
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl BenchApp {
    /// A reference into the open thread's composer (once).
    pub fn add_reference(&mut self, r: Reference, cx: &mut Context<Self>) {
        let key = self.thread_ref(cx).key();
        let list = self.references.entry(key).or_default();
        if !list.contains(&r) {
            list.push(r);
        }
        cx.notify();
    }
}

impl ThreadView {
    /// This thread's references waiting in its composer (kept by the app per thread, as drafts).
    pub fn thread_references(&self, cx: &App) -> Vec<Reference> {
        let key = self.thread_ref().key();
        self.app(cx)
            .and_then(|a| a.references.get(&key).cloned())
            .unwrap_or_default()
    }

    pub fn remove_reference(&mut self, k: usize, cx: &mut Context<Self>) {
        let key = self.thread_ref().key();
        self.app
            .update(cx, |app, cx| {
                if let Some(list) = app.references.get_mut(&key) {
                    if k < list.len() {
                        list.remove(k);
                    }
                }
                cx.notify();
            })
            .ok();
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
    use super::{CodeRef, ProblemsRef, Reference, TerminalRef, lines_of, with_references};

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
        assert_eq!(
            with_references("look".into(), &[Reference::Code(f)]),
            "look\n\n`/p/README`"
        );
    }

    #[test]
    fn problems_read_as_lines_under_the_words() {
        let p = ProblemsRef {
            of: Some("/p/src/a.rs".into()),
            lines: vec!["/p/src/a.rs:2:9 error: found ERROR (fake)".into()],
        };
        assert_eq!(p.label(), "Problems · a.rs (1)");
        assert_eq!(
            with_references(String::new(), &[Reference::Problems(p)]),
            "Problems from the language servers in `/p/src/a.rs`:\n```\n/p/src/a.rs:2:9 error: found ERROR (fake)\n```"
        );
        let all = ProblemsRef {
            of: None,
            lines: vec!["x".into(), "y".into()],
        };
        assert_eq!(all.label(), "Problems (2)");
    }

    #[test]
    fn a_terminal_reads_as_its_lines_fenced() {
        let t = TerminalRef {
            machine: "dev".into(),
            text: "$ make\nok\n".into(),
        };
        assert_eq!(t.label(), "Terminal · dev (2 lines)");
        assert_eq!(
            Reference::Terminal(t).message(),
            "The terminal on dev shows:\n```\n$ make\nok\n```"
        );
    }
}
