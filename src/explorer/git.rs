//! Git at a glance in the Explorer, as Zed's gutter has it: a file's lines changed since the last
//! commit marked beside its line numbers (added, changed, removed), and its diff against HEAD as
//! a tab of its own (`DocKey::Diff`), read only. HEAD's text comes from the agent
//! (`/api/git/head`, on the file's machine); the comparison is made here, as the file is typed.

use gpui_base::input::LineMark;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;
use similar::{DiffOp, TextDiff};

use super::{DocKey, FileRef};
use crate::app::BenchApp;
use crate::client::rest;

/// What happened to lines since HEAD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hunk {
    /// These lines (0-based, of the text now) are new.
    Added(std::ops::Range<usize>),
    /// These lines replaced others.
    Changed(std::ops::Range<usize>),
    /// Lines were removed before this one.
    Removed(usize),
}

/// The text now against HEAD's, line by line.
pub fn hunks(head: &str, now: &str) -> Vec<Hunk> {
    TextDiff::from_lines(head, now)
        .ops()
        .iter()
        .filter_map(|op| match *op {
            DiffOp::Equal { .. } => None,
            DiffOp::Insert {
                new_index, new_len, ..
            } => Some(Hunk::Added(new_index..new_index + new_len)),
            DiffOp::Delete { new_index, .. } => Some(Hunk::Removed(new_index)),
            DiffOp::Replace {
                new_index, new_len, ..
            } => Some(Hunk::Changed(new_index..new_index + new_len)),
        })
        .collect()
}

/// The diff against HEAD as `git diff` prints it, three lines around each change.
pub fn unified(path: &str, head: &str, now: &str) -> String {
    TextDiff::from_lines(head, now)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{path}"), &format!("b/{path}"))
        .to_string()
}

impl BenchApp {
    /// HEAD's text of an open file, read once it is open; then its changes are marked.
    pub(super) fn read_head(&mut self, f: &FileRef, window: &mut Window, cx: &mut Context<Self>) {
        if self.demo {
            if let Some(d) = self.explorer.docs.get_mut(&DocKey::File(f.clone())) {
                d.head = crate::demo::file_head(&f.path);
            }
            self.mark_changes(f, window, cx);
            return;
        }
        let Some((conn, _)) = self.env_target(&f.project) else {
            return;
        };
        let profile = self.current_profile();
        let (machine, path, f) = (f.machine.clone(), f.path.clone(), f.clone());
        self.background(
            window,
            cx,
            move || rest::git_head(&conn, &profile, machine.as_deref(), &path),
            move |this, r, window, cx| {
                // Not a repository, or the agent could not tell: nothing is marked.
                if let Some(d) = this.explorer.docs.get_mut(&DocKey::File(f.clone())) {
                    d.head = r.ok().flatten();
                }
                this.mark_changes(&f, window, cx);
            },
        );
    }

    /// The file's changed lines marked in its gutter, and its diff tab (when open) written anew.
    pub(super) fn mark_changes(&mut self, f: &FileRef, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.explorer.docs.get(&DocKey::File(f.clone())) else {
            return;
        };
        let theme = cx.theme().clone();
        let now = d.editor.read(cx).value().to_string();
        let (marks, diff) = match &d.head {
            Some(head) => {
                let marks = hunks(head, &now)
                    .into_iter()
                    .map(|h| match h {
                        Hunk::Added(lines) => LineMark {
                            lines,
                            color: theme.success,
                        },
                        Hunk::Changed(lines) => LineMark {
                            lines,
                            color: theme.warning,
                        },
                        Hunk::Removed(at) => LineMark {
                            lines: at..at,
                            color: theme.danger,
                        },
                    })
                    .collect();
                (marks, unified(f.name(), head, &now))
            }
            None => (vec![], String::new()),
        };
        d.editor.update(cx, |s, cx| s.set_line_marks(marks, cx));
        if let Some(view) = self.explorer.docs.get_mut(&DocKey::Diff(f.clone())) {
            view.loading = false;
            let text = if diff.is_empty() {
                "No changes since the last commit\n".to_string()
            } else {
                diff
            };
            view.editor.update(cx, |s, cx| s.set_value(text, window, cx));
        }
    }

    /// A file's diff against HEAD in a tab of its own.
    pub fn open_diff(&mut self, f: FileRef, window: &mut Window, cx: &mut Context<Self>) {
        self.open_doc(DocKey::Diff(f), window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{Hunk, hunks, unified};

    #[test]
    fn lines_since_head_are_added_changed_or_removed() {
        let head = "a\nb\nc\nd\n";
        assert_eq!(hunks(head, head), vec![]);
        assert_eq!(hunks(head, "a\nB\nc\nd\n"), vec![Hunk::Changed(1..2)]);
        assert_eq!(hunks(head, "a\nb\nx\ny\nc\nd\n"), vec![Hunk::Added(2..4)]);
        assert_eq!(hunks(head, "a\nd\n"), vec![Hunk::Removed(1)]);
        let d = unified("f.txt", head, "a\nB\nc\nd\n");
        assert!(d.starts_with("--- a/f.txt\n+++ b/f.txt\n@@ -1,4 +1,4 @@\n"), "{d}");
        assert!(d.contains("-b\n+B\n"));
    }
}
