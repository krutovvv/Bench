//! The mini IDE's navigation: ⌃G to a line, ⌘⇧F to find text in every file under the tree's
//! root (results by file, a click opens the file at that line), the open file following the
//! agent's edits, and ⌘W / ⌃Tab over the tabs. On this Mac with std::fs, on a server with grep
//! over ssh.

use std::time::Duration;

use anyhow::Result;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::BenchApp;
use super::ide_ops::{Fs, SKIP_DIRS, list_files};
use crate::app::RightTab;
use crate::ssh::{q, run};

/// ⌃G: a line number being typed.
pub struct GoLine {
    pub input: Entity<InputState>,
    _sub: Subscription,
}

/// The tree's folder as a typed path (Enter goes there, Esc cancels).
pub struct RootEdit {
    pub input: Entity<InputState>,
    _sub: Subscription,
}

/// One line that matched.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub path: String,
    /// 1-based.
    pub line: usize,
    pub text: String,
}

/// ⌘⇧F: the query, and what it found (kept while the tab is away).
pub struct Find {
    pub input: Entity<InputState>,
    pub hits: Vec<Hit>,
    pub running: bool,
    /// What `hits` were found for ("" before the first search).
    pub searched: String,
    /// Results stopped at MAX_HITS.
    pub capped: bool,
    _sub: Subscription,
}

const MAX_HITS: usize = 2000;
/// Bigger files are not searched (dumps, bundles).
const MAX_FILE: u64 = 4 << 20;

/// Case-insensitive unless the query has a capital (smart case, like Zed's default).
fn matcher(query: &str) -> impl Fn(&str) -> bool + '_ {
    let smart = query.chars().any(char::is_uppercase);
    let lower = query.to_lowercase();
    move |line: &str| {
        if smart {
            line.contains(query)
        } else {
            line.to_lowercase().contains(&lower)
        }
    }
}

fn clip_line(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 240 {
        format!("{}…", t.chars().take(240).collect::<String>())
    } else {
        t.to_string()
    }
}

/// Every line under `root` containing `query`, file by file.
fn grep(fs: &Fs, root: &str, query: &str) -> Result<(Vec<Hit>, bool)> {
    let mut hits = vec![];
    match fs {
        Fs::Ssh(alias) => {
            let excludes: String = SKIP_DIRS
                .iter()
                .map(|d| format!(" --exclude-dir={}", q(d)))
                .collect();
            let smart = if query.chars().any(char::is_uppercase) {
                ""
            } else {
                " -i"
            };
            let cmd = format!(
                "grep -rnIF{smart}{excludes} -- {} {} 2>/dev/null | head -n {}",
                q(query),
                q(root),
                MAX_HITS + 1
            );
            let out = run(alias, &cmd, None, Duration::from_secs(60))?;
            for l in String::from_utf8_lossy(&out).lines() {
                let mut parts = l.splitn(3, ':');
                let (Some(path), Some(n), Some(text)) = (parts.next(), parts.next(), parts.next())
                else {
                    continue;
                };
                let Ok(line) = n.parse() else { continue };
                hits.push(Hit {
                    path: path.to_string(),
                    line,
                    text: clip_line(text),
                });
            }
        }
        Fs::Local => {
            let is_match = matcher(query);
            for path in list_files(fs, root)? {
                let big = std::fs::metadata(&path).map_or(true, |m| m.len() > MAX_FILE);
                if big {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if bytes.contains(&0) {
                    continue; // binary
                }
                let text = String::from_utf8_lossy(&bytes);
                for (i, l) in text.lines().enumerate() {
                    if is_match(l) {
                        hits.push(Hit {
                            path: path.clone(),
                            line: i + 1,
                            text: clip_line(l),
                        });
                        if hits.len() > MAX_HITS {
                            break;
                        }
                    }
                }
                if hits.len() > MAX_HITS {
                    break;
                }
            }
        }
    }
    let capped = hits.len() > MAX_HITS;
    hits.truncate(MAX_HITS);
    Ok((hits, capped))
}

impl BenchApp {
    // ---- ⌃G ------------------------------------------------------------------

    pub fn open_go_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file.is_none() {
            return;
        }
        if self.ide.goto.is_some() {
            self.ide.goto = None;
            cx.notify();
            return;
        }
        let total = self.file_editor.read(cx).value().matches('\n').count() + 1;
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("Go to line (1–{total}), or line:column"))
        });
        let sub = cx.subscribe_in(
            &input,
            window,
            |this, state, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    let text = state.read(cx).value().trim().to_string();
                    this.ide.goto = None;
                    let mut parts = text.split([':', ',']);
                    let line = parts.next().and_then(|n| n.trim().parse::<usize>().ok());
                    let col = parts.next().and_then(|n| n.trim().parse::<usize>().ok());
                    if let Some(line) = line {
                        this.go_to_line(line, col.unwrap_or(1), window, cx);
                    }
                    cx.notify();
                }
            },
        );
        input.update(cx, |s, cx| s.focus(window, cx));
        self.ide.goto = Some(GoLine { input, _sub: sub });
        cx.notify();
    }

    /// Put the cursor on `line` (1-based) at `col` (1-based), scrolled into view.
    pub fn go_to_line(
        &mut self,
        line: usize,
        col: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.file_editor.update(cx, |s, cx| {
            let last = s.value().matches('\n').count();
            let position = gpui_kit::component::input::Position::new(
                line.saturating_sub(1).min(last) as u32,
                col.saturating_sub(1) as u32,
            );
            s.set_cursor_position(position, window, cx);
        });
    }

    /// A line asked for before its file had loaded (a search result): go there now.
    pub(super) fn apply_pending_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.file.as_ref().map(|f| f.path.clone());
        if let Some((p, line)) = self.ide.pending_line.take() {
            if Some(&p) == path.as_ref() {
                self.go_to_line(line, 1, window, cx);
            } else {
                self.ide.pending_line = Some((p, line));
            }
        }
    }

    // ---- the tree's folder -------------------------------------------------------

    /// Like Transmit: the tree may go anywhere on the machine, not just below where it started.
    pub fn go_to_folder(&mut self, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        self.ide.root_edit = None;
        let dir = match dir.trim() {
            "" => return,
            "/" => "/".to_string(),
            d => d.trim_end_matches('/').to_string(),
        };
        let dir = match dir.strip_prefix('~') {
            // A local `~`; a server's `~` stays for the server to expand.
            Some(rest)
                if self.ide.server.is_none() && self.target == crate::model::Target::Local =>
            {
                format!("{}{rest}", std::env::var("HOME").unwrap_or_default())
            }
            _ => dir,
        };
        self.set_tree_root(dir, window, cx);
        cx.notify();
    }

    /// The root row's pencil: type the folder to show.
    pub fn start_root_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.tree_root.clone().unwrap_or_default();
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("/etc/nginx  ·  ~/site  ·  Enter");
            s.set_value(current, window, cx);
            s
        });
        let sub = cx.subscribe_in(
            &input,
            window,
            |this, state, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => {
                    let dir = state.read(cx).value().to_string();
                    this.go_to_folder(dir, window, cx);
                }
                InputEvent::Blur => {
                    this.ide.root_edit = None;
                    cx.notify();
                }
                _ => {}
            },
        );
        input.update(cx, |s, cx| s.focus(window, cx));
        self.ide.root_edit = Some(RootEdit { input, _sub: sub });
        cx.notify();
    }

    // ---- ⌘⇧F -----------------------------------------------------------------

    /// Show the Search tab with its field focused (the last results stay).
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tree_root.is_none() {
            self.toast(
                NotificationType::Info,
                "Open files first (⌘B, or Open files in a ⋯ menu)",
                window,
                cx,
            );
            return;
        }
        if self.ide.find.is_none() {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Find in files — Enter"));
            let sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.run_find(window, cx);
                }
            });
            self.ide.find = Some(Find {
                input,
                hits: vec![],
                running: false,
                searched: String::new(),
                capped: false,
                _sub: sub,
            });
        }
        // The selected text, if any, is what to look for.
        let selected = self.file_editor.read(cx).selected_text().to_string();
        if let Some(f) = self.ide.find.as_ref() {
            let input = f.input.clone();
            input.update(cx, |s, cx| {
                if !selected.trim().is_empty() && !selected.contains('\n') {
                    s.set_value(selected.trim().to_string(), window, cx);
                }
                s.focus(window, cx);
            });
        }
        self.right = Some(RightTab::Search);
        cx.notify();
    }

    pub fn run_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(f), Some(root)) = (self.ide.find.as_mut(), self.tree_root.clone()) else {
            return;
        };
        let query = f.input.read(cx).value().trim().to_string();
        if query.is_empty() || f.running {
            return;
        }
        f.running = true;
        let fs = self.fs();
        let q2 = query.clone();
        self.background(
            window,
            cx,
            move || grep(&fs, &root, &q2),
            move |this, r, window, cx| {
                let Some(f) = this.ide.find.as_mut() else {
                    return;
                };
                f.running = false;
                match r {
                    Ok((hits, capped)) => {
                        f.hits = hits;
                        f.capped = capped;
                        f.searched = query;
                    }
                    Err(e) => {
                        this.toast(NotificationType::Error, format!("Find: {e:#}"), window, cx)
                    }
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    /// A result: its file, at its line.
    pub fn open_hit(&mut self, hit: Hit, window: &mut Window, cx: &mut Context<Self>) {
        let already = self
            .file
            .as_ref()
            .is_some_and(|f| f.path == hit.path && f.loaded);
        if already {
            self.go_to_line(hit.line, 1, window, cx);
            return;
        }
        self.ide.pending_line = Some((hit.path.clone(), hit.line));
        self.open_file(hit.path, window, cx);
        // A parked tab comes back at once.
        self.apply_pending_line(window, cx);
    }

    // ---- the agent edits the open file ----------------------------------------

    /// A tool wrote `path`: the open file follows unless it has unsaved edits (then a bar
    /// offers to reload). Parked tabs without edits are dropped, so they reload when shown.
    pub(super) fn file_changed_outside(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ide.server.is_some() {
            return; // a server file's local copy is watched already
        }
        if let Some(p) = self.ide.parked.get(path) {
            if !p.dirty {
                self.ide.parked.remove(path);
            }
        }
        let open = self.file.as_ref().is_some_and(|f| f.path == path);
        if !open {
            return;
        }
        if self.ide.dirty {
            self.ide.changed_outside = true;
            cx.notify();
        } else {
            self.reload_open_file(window, cx);
        }
    }

    /// Read the open file again, keeping the cursor's line.
    pub fn reload_open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.file.as_ref().map(|f| f.path.clone()) else {
            return;
        };
        self.ide.changed_outside = false;
        let line = self.file_editor.read(cx).cursor_position().line as usize + 1;
        let p = path.clone();
        self.rest(
            window,
            cx,
            move |c, prof| crate::rest::read_text(c, prof, &p),
            move |this, r, window, cx| {
                let Ok(file) = r else { return };
                if file.binary || this.file.as_ref().map(|f| f.path.as_str()) != Some(&path) {
                    return;
                }
                if let Some(view) = this.file.as_mut() {
                    view.truncated = file.truncated;
                }
                this.ide.baseline = file.text.clone();
                this.ide.dirty = false;
                this.file_editor
                    .update(cx, |s, cx| s.set_value(file.text, window, cx));
                this.go_to_line(line, 1, window, cx);
                cx.notify();
            },
        );
    }

    // ---- tabs from the keyboard ------------------------------------------------

    /// ⌘W: close the active tab (twice when it has unsaved edits, as its ×).
    pub fn close_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.file.as_ref().map(|f| f.path.clone()) {
            self.close_tab(path, window, cx);
        }
    }

    /// ⌃Tab / ⌃⇧Tab: the next / previous tab, wrapping.
    pub fn cycle_tab(&mut self, step: i32, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.ide.tabs.len() as i32;
        if n < 2 {
            return;
        }
        let at = self
            .file
            .as_ref()
            .and_then(|f| self.ide.tabs.iter().position(|t| *t == f.path))
            .unwrap_or(0) as i32;
        let next = self.ide.tabs[(at + step).rem_euclid(n) as usize].clone();
        self.switch_tab(next, window, cx);
    }

    /// ⌥Z: wrap long lines or not (remembered for the window).
    pub fn toggle_soft_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ide.wrap = !self.ide.wrap;
        let wrap = self.ide.wrap;
        self.file_editor
            .update(cx, |s, cx| s.set_soft_wrap(wrap, window, cx));
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{Fs, grep, matcher};

    #[test]
    fn smart_case() {
        let m = matcher("foo");
        assert!(m("a FOO b"));
        let m = matcher("Foo");
        assert!(m("a Foo b") && !m("a foo b"));
    }

    #[test]
    fn local_grep_finds_lines() {
        let dir = std::env::temp_dir().join(format!("bench-grep-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        std::fs::write(dir.join("a.rs"), "fn main() {\n    let needle = 1;\n}\n").unwrap();
        std::fs::write(dir.join("node_modules/b.js"), "needle").unwrap();
        let (hits, capped) = grep(&Fs::Local, &dir.to_string_lossy(), "NEEDLE").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(!capped);
        assert_eq!(hits.len(), 0, "smart case: a capital means exact case");
        let dir = std::env::temp_dir().join(format!("bench-grep2-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        std::fs::write(dir.join("a.rs"), "fn main() {\n    let needle = 1;\n}\n").unwrap();
        std::fs::write(dir.join("node_modules/b.js"), "needle").unwrap();
        let (hits, _) = grep(&Fs::Local, &dir.to_string_lossy(), "needle").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        assert_eq!(hits[0].text, "let needle = 1;");
    }
}
