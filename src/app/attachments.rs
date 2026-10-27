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

/// The message's words with the references under them.
pub fn with_references(text: String, refs: &[CodeRef]) -> String {
    std::iter::once(text)
        .filter(|t| !t.is_empty())
        .chain(refs.iter().map(CodeRef::message))
        .collect::<Vec<_>>()
        .join("\n\n")
}

impl BenchApp {
    /// A reference to the project's code into the composer (once).
    pub fn add_reference(&mut self, r: CodeRef, cx: &mut Context<Self>) {
        if !self.references.contains(&r) {
            self.references.push(r);
        }
        cx.notify();
    }

    pub fn remove_reference(&mut self, k: usize, cx: &mut Context<Self>) {
        if k < self.references.len() {
            self.references.remove(k);
            cx.notify();
        }
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
