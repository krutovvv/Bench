//! Code from the mini IDE in the chat, as Zed's agent panel does it: a selection goes to the
//! message as a chip "file.rs:12–20" (⌘⇧. or the toolbar's button), and text copied in the
//! editor pastes into the message as such a chip, not as bare text. Sent, a chip is a block the
//! agent reads — `[selection: <path>:<start>-<end>]` and the code fenced with its language —
//! and in the chat it shows as the chip again; a click opens the file there.

use gpui_kit::*;

use super::BenchApp;
use crate::app::ide_find::Hit;

pub const PREFIX: &str = "[selection: ";

/// Lines of one file going with a message.
#[derive(Clone, Debug, PartialEq)]
pub struct Snippet {
    /// The file (a server's as `alias:/path`).
    pub path: String,
    /// 1-based, inclusive.
    pub start: usize,
    pub end: usize,
    pub lang: String,
    pub text: String,
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// "main.rs:12–20" (one line: "main.rs:12").
pub fn label(path: &str, start: usize, end: usize) -> String {
    if start == end {
        format!("{}:{start}", file_name(path))
    } else {
        format!("{}:{start}–{end}", file_name(path))
    }
}

impl Snippet {
    /// What the agent reads.
    pub fn block(&self) -> String {
        let fence = if self.text.contains("```") {
            "````"
        } else {
            "```"
        };
        format!(
            "{PREFIX}{}:{}-{}]\n{fence}{}\n{}\n{fence}",
            self.path,
            self.start,
            self.end,
            self.lang.to_lowercase(),
            self.text.trim_end_matches('\n')
        )
    }
}

/// A sent message's selections as (path, start, end), and its text without their blocks.
pub fn split(text: &str) -> (String, Vec<(String, usize, usize)>) {
    let mut refs = vec![];
    let mut out: Vec<&str> = vec![];
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(r) = line
            .trim()
            .strip_prefix(PREFIX)
            .and_then(|r| r.strip_suffix(']'))
        else {
            out.push(line);
            continue;
        };
        let parsed = r.rsplit_once(':').and_then(|(path, range)| {
            let (a, b) = range.split_once('-').unwrap_or((range, range));
            Some((path.to_string(), a.parse().ok()?, b.parse().ok()?))
        });
        let Some(parsed) = parsed else {
            out.push(line);
            continue;
        };
        // Skip the fenced block that follows.
        if let Some(open) = lines.peek().map(|l| l.trim().to_string()) {
            let fence: String = open.chars().take_while(|c| *c == '`').collect();
            if fence.len() >= 3 {
                lines.next();
                for l in lines.by_ref() {
                    if l.trim() == fence {
                        break;
                    }
                }
            }
        }
        refs.push(parsed);
    }
    (out.join("\n").trim().to_string(), refs)
}

impl BenchApp {
    /// The editor's selection (or the cursor's line) as a snippet of the open file.
    fn selection_snippet(&self, cx: &App) -> Option<Snippet> {
        let view = self.file.as_ref()?;
        let s = self.file_editor.read(cx);
        let text = s.value().to_string();
        let mut range = s.selected_range();
        if range.is_empty() {
            // Nothing selected: the cursor's line.
            let at = range.start.min(text.len());
            let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
            let end = text[at..].find('\n').map_or(text.len(), |i| at + i);
            range = start..end;
        }
        let picked = text.get(range.clone())?.to_string();
        if picked.trim().is_empty() {
            return None;
        }
        let start = text[..range.start].matches('\n').count() + 1;
        let end = start + picked.trim_end_matches('\n').matches('\n').count();
        let path = match &self.ide.server {
            Some(alias) => format!("{alias}:{}", view.path),
            None => view.path.clone(),
        };
        Some(Snippet {
            path,
            start,
            end,
            lang: s.language_name().to_string(),
            text: picked,
        })
    }

    /// ⌘⇧.: the selection goes to the message being written.
    pub fn add_selection_to_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snippet) = self.selection_snippet(cx) else {
            return;
        };
        if !self.snippets.contains(&snippet) {
            self.snippets.push(snippet);
        }
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// ⌘C in the editor: remember where the copied text came from (the editor copies it).
    pub fn note_copy(&mut self, cx: &mut Context<Self>) {
        self.ide.copied = self
            .selection_snippet(cx)
            .filter(|_| !self.file_editor.read(cx).selected_range().is_empty());
    }

    /// ⌘V in the message: the editor's last copy, still on the clipboard, comes as a chip.
    pub fn paste_snippet(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(copied) = self.ide.copied.clone() else {
            return false;
        };
        let on_clipboard = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .is_some_and(|t| t == copied.text);
        if !on_clipboard {
            return false;
        }
        if !self.snippets.contains(&copied) {
            self.snippets.push(copied);
        }
        cx.notify();
        true
    }

    pub fn remove_snippet(&mut self, k: usize, cx: &mut Context<Self>) {
        if k < self.snippets.len() {
            self.snippets.remove(k);
            cx.notify();
        }
    }

    /// A chip in a sent message: its file in the IDE, at its first line.
    pub fn open_snippet(
        &mut self,
        path: String,
        line: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (server, file) = match path.split_once(':') {
            Some((alias, p)) if p.starts_with('/') && !path.starts_with('/') => {
                (Some(alias.to_string()), p.to_string())
            }
            _ => (None, path),
        };
        let dir = file
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .filter(|d| !d.is_empty());
        let same_place = self.ide_open() && self.ide.server == server;
        if !same_place {
            match server {
                Some(alias) => self.open_ide_server_at(alias, dir, window, cx),
                None => self.open_ide(self.tree_root.clone().or(dir), window, cx),
            }
        }
        self.open_hit(
            Hit {
                path: file,
                line,
                text: String::new(),
            },
            window,
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Snippet, label, split};

    #[test]
    fn block_round_trips() {
        let s = Snippet {
            path: "/p/src/main.rs".into(),
            start: 12,
            end: 14,
            lang: "Rust".into(),
            text: "fn a() {}\nfn b() {}\nfn c() {}\n".into(),
        };
        let msg = format!("Why?\n\n{}\n\nthanks", s.block());
        let (text, refs) = split(&msg);
        assert_eq!(text, "Why?\n\n\nthanks");
        assert_eq!(refs, vec![("/p/src/main.rs".to_string(), 12, 14)]);
        assert_eq!(label("/p/src/main.rs", 12, 14), "main.rs:12–14");
        assert!(s.block().contains("```rust\nfn a() {}"));
    }

    #[test]
    fn server_paths_keep_the_alias() {
        let (_, refs) = split("[selection: web:/srv/app.py:3-3]\n```python\nx = 1\n```");
        assert_eq!(refs, vec![("web:/srv/app.py".to_string(), 3, 3)]);
    }
}
