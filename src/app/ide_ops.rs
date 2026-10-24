//! The mini IDE's file work beyond open and save: ⌘P to find a file under the tree's root, new
//! file / new folder / rename / delete from a tree row's menu, refreshing the tree, revealing
//! the open file in it. On this Mac with std::fs, on a server with ssh (`ssh.rs`).

use std::time::Duration;

use anyhow::{Result, bail};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::HermesApp;
use crate::model::Target;
use crate::ssh::{q, run};

/// Where the tree's files are, for file operations.
#[derive(Clone)]
pub(super) enum Fs {
    Local,
    Ssh(String),
}

/// ⌘P: a query over every file under the root (listed once when it opens).
pub struct Quick {
    pub input: Entity<InputState>,
    /// None while listing.
    pub files: Option<Vec<String>>,
    pub sel: usize,
    _sub: Subscription,
}

#[derive(Clone, PartialEq)]
pub enum EditKind {
    NewFile(String),
    NewDir(String),
    Rename(String),
}

/// A name being typed in the tree (Enter applies, Esc cancels).
pub struct TreeEdit {
    pub kind: EditKind,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

const MAX_LISTED: usize = 20_000;
pub(super) const SKIP_DIRS: [&str; 6] = [
    ".git",
    "node_modules",
    "target",
    ".venv",
    "__pycache__",
    ".cache",
];

fn parent(path: &str) -> String {
    match path.trim_end_matches('/').rsplit_once('/') {
        Some(("", _)) => "/".into(),
        Some((p, _)) => p.into(),
        None => ".".into(),
    }
}

pub(super) fn list_files(fs: &Fs, root: &str) -> Result<Vec<String>> {
    match fs {
        Fs::Ssh(alias) => {
            let prune = SKIP_DIRS
                .iter()
                .map(|d| format!("-name {}", q(d)))
                .collect::<Vec<_>>()
                .join(" -o ");
            let cmd = format!(
                "find {} \\( {prune} \\) -prune -o -type f -print 2>/dev/null | head -n {MAX_LISTED}",
                q(root)
            );
            let out = run(alias, &cmd, None, Duration::from_secs(60))?;
            Ok(String::from_utf8_lossy(&out)
                .lines()
                .map(str::to_string)
                .collect())
        }
        Fs::Local => {
            let mut out = vec![];
            let mut stack = vec![std::path::PathBuf::from(root)];
            while let Some(dir) = stack.pop() {
                let Ok(rd) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let Ok(ft) = e.file_type() else { continue };
                    if ft.is_dir() {
                        if !SKIP_DIRS.contains(&name.as_str()) {
                            stack.push(e.path());
                        }
                    } else if ft.is_file() {
                        out.push(e.path().to_string_lossy().to_string());
                        if out.len() >= MAX_LISTED {
                            return Ok(out);
                        }
                    }
                }
            }
            Ok(out)
        }
    }
}

fn apply(fs: &Fs, kind: &EditKind, name: &str) -> Result<String> {
    if name.is_empty() || name.contains('/') || name == "." || name == ".." {
        bail!("not a file name: {name:?}");
    }
    let (target, from) = match kind {
        EditKind::NewFile(dir) | EditKind::NewDir(dir) => {
            (format!("{}/{name}", dir.trim_end_matches('/')), None)
        }
        EditKind::Rename(path) => (format!("{}/{name}", parent(path)), Some(path.clone())),
    };
    match fs {
        Fs::Local => {
            let t = std::path::Path::new(&target);
            if t.exists() {
                bail!("{name} already exists");
            }
            match (kind, &from) {
                (EditKind::NewFile(_), _) => {
                    std::fs::write(t, b"")?;
                }
                (EditKind::NewDir(_), _) => std::fs::create_dir(t)?,
                (EditKind::Rename(_), Some(from)) => std::fs::rename(from, t)?,
                _ => {}
            }
        }
        Fs::Ssh(alias) => {
            let t = q(&target);
            let cmd = match (kind, &from) {
                (EditKind::NewFile(_), _) => format!("[ ! -e {t} ] && : > {t}"),
                (EditKind::NewDir(_), _) => format!("[ ! -e {t} ] && mkdir {t}"),
                (EditKind::Rename(_), Some(from)) => format!("[ ! -e {t} ] && mv {} {t}", q(from)),
                _ => bail!("nothing to do"),
            };
            run(alias, &cmd, None, Duration::from_secs(30))
                .map_err(|e| anyhow::anyhow!("{name}: already exists or not allowed ({e})"))?;
        }
    }
    Ok(target)
}

fn delete(fs: &Fs, path: &str) -> Result<()> {
    match fs {
        Fs::Local => {
            let p = std::path::Path::new(path);
            if p.is_dir() {
                std::fs::remove_dir_all(p)?
            } else {
                std::fs::remove_file(p)?
            }
        }
        Fs::Ssh(alias) => {
            run(
                alias,
                &format!("rm -rf -- {}", q(path)),
                None,
                Duration::from_secs(60),
            )?;
        }
    }
    Ok(())
}

/// Characters of `query` in order within `path`: a score (higher is better), None if absent.
/// Hits in the file name and runs of adjacent characters count more.
pub fn fuzzy(query: &str, path: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let p = path.to_lowercase();
    let name_start = p.rfind('/').map_or(0, |i| i + 1);
    let mut score = 0;
    let mut pos = 0;
    let mut last: Option<usize> = None;
    for qc in query.to_lowercase().chars() {
        let found = p[pos..].find(qc)? + pos;
        score += if found >= name_start { 3 } else { 1 };
        if last.is_some_and(|l| l + 1 == found) {
            score += 4;
        }
        last = Some(found);
        pos = found + qc.len_utf8();
    }
    Some(score * 100 - p.len() as i32)
}

impl HermesApp {
    pub(super) fn fs(&self) -> Fs {
        match (&self.ide.server, &self.target) {
            (Some(alias), _) => Fs::Ssh(alias.clone()),
            (None, Target::Local) => Fs::Local,
            (None, Target::Ssh(host)) => Fs::Ssh(host.clone()),
        }
    }

    // ---- ⌘P ------------------------------------------------------------------

    pub fn open_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ide.quick.is_some() {
            self.ide.quick = None;
            cx.notify();
            return;
        }
        let Some(root) = self.tree_root.clone() else {
            self.toast(
                NotificationType::Info,
                "Open files first (⌘B, or Open files in a ⋯ menu)",
                window,
                cx,
            );
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find a file"));
        let sub = cx.subscribe_in(
            &input,
            window,
            |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => {
                    if let Some(q) = this.ide.quick.as_mut() {
                        q.sel = 0;
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.quick_pick(None, window, cx),
                _ => {}
            },
        );
        input.update(cx, |s, cx| s.focus(window, cx));
        self.ide.quick = Some(Quick {
            input,
            files: None,
            sel: 0,
            _sub: sub,
        });
        let fs = self.fs();
        self.background(
            window,
            cx,
            move || list_files(&fs, &root),
            |this, r, window, cx| match r {
                Ok(files) => {
                    if let Some(q) = this.ide.quick.as_mut() {
                        q.files = Some(files);
                    }
                    cx.notify();
                }
                Err(e) => {
                    this.ide.quick = None;
                    this.toast(NotificationType::Error, format!("Files: {e:#}"), window, cx);
                }
            },
        );
        cx.notify();
    }

    /// The best matches for what is typed (paths), at most `n`.
    pub fn quick_matches(&self, n: usize, cx: &App) -> Vec<String> {
        let Some(q) = self.ide.quick.as_ref() else {
            return vec![];
        };
        let Some(files) = q.files.as_ref() else {
            return vec![];
        };
        let query = q.input.read(cx).value().trim().to_string();
        let root = self.tree_root.clone().unwrap_or_default();
        // Nothing typed: the open tabs first (the last opened on top), then the rest.
        if query.is_empty() {
            let tabs: Vec<String> = self.ide.tabs.iter().rev().cloned().collect();
            return tabs
                .iter()
                .cloned()
                .chain(files.iter().filter(|f| !tabs.contains(f)).cloned())
                .take(n)
                .collect();
        }
        let mut hits: Vec<(i32, &String)> = files
            .iter()
            .filter_map(|f| {
                let rel = f.strip_prefix(&root).unwrap_or(f);
                fuzzy(&query, rel).map(|s| (s, f))
            })
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0));
        hits.into_iter().take(n).map(|(_, f)| f.clone()).collect()
    }

    pub fn quick_move(&mut self, step: i32, cx: &mut Context<Self>) {
        let n = self.quick_matches(50, cx).len() as i32;
        if let Some(q) = self.ide.quick.as_mut() {
            if n > 0 {
                q.sel = (q.sel as i32 + step).rem_euclid(n) as usize;
            }
        }
        cx.notify();
    }

    /// Open the chosen match (`path`, else the selected one), showing it in the tree.
    pub fn quick_pick(
        &mut self,
        path: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = path.or_else(|| {
            let sel = self.ide.quick.as_ref().map_or(0, |q| q.sel);
            self.quick_matches(50, cx).get(sel).cloned()
        });
        self.ide.quick = None;
        if let Some(path) = path {
            self.reveal(&path, window, cx);
            self.open_file(path, window, cx);
        }
        cx.notify();
    }

    /// Open the folders between the root and `path`, so the file shows in the tree.
    pub fn reveal(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.tree_root.clone() else {
            return;
        };
        let root = root.trim_end_matches('/').to_string();
        let mut dirs = vec![];
        let mut dir = parent(path);
        while dir.len() > root.len() && dir.starts_with(&root) {
            dirs.push(dir.clone());
            dir = parent(&dir);
        }
        for d in dirs.into_iter().rev() {
            if !self.tree_open.contains(&d) {
                self.open_dir(d, window, cx);
            }
        }
    }

    // ---- the tree's files -----------------------------------------------------------

    /// List `dir` again (after a change in it), keeping it open.
    fn relist(&mut self, dir: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tree.remove(dir);
        self.tree_open.remove(dir);
        self.open_dir(dir.to_string(), window, cx);
    }

    /// The tree as it is on disk now: every open folder listed again.
    pub fn refresh_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut open: Vec<String> = self.tree_open.iter().cloned().collect();
        open.sort_by_key(|d| d.len());
        self.tree.clear();
        self.tree_open.clear();
        for d in open {
            self.open_dir(d, window, cx);
        }
        cx.notify();
    }

    pub fn start_tree_edit(&mut self, kind: EditKind, window: &mut Window, cx: &mut Context<Self>) {
        let initial = match &kind {
            EditKind::Rename(path) => path.rsplit('/').next().unwrap_or_default().to_string(),
            _ => String::new(),
        };
        if let EditKind::NewFile(dir) | EditKind::NewDir(dir) = &kind {
            if !self.tree_open.contains(dir) && self.tree_root.as_deref() != Some(dir.as_str()) {
                self.open_dir(dir.clone(), window, cx);
            }
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(match &kind {
                EditKind::NewFile(_) => "File name",
                EditKind::NewDir(_) => "Folder name",
                EditKind::Rename(_) => "New name",
            })
        });
        input.update(cx, |s, cx| {
            s.set_value(initial, window, cx);
            s.focus(window, cx);
        });
        let sub = cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.apply_tree_edit(window, cx);
            }
        });
        self.ide.edit = Some(TreeEdit {
            kind,
            input,
            _sub: sub,
        });
        cx.notify();
    }

    fn apply_tree_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.ide.edit.take() else {
            return;
        };
        let name = edit.input.read(cx).value().trim().to_string();
        let kind = edit.kind.clone();
        if let EditKind::Rename(path) = &kind {
            if path.rsplit('/').next() == Some(name.as_str()) {
                cx.notify();
                return;
            }
        }
        let fs = self.fs();
        let k = kind.clone();
        self.background(
            window,
            cx,
            move || apply(&fs, &k, &name),
            move |this, r, window, cx| match r {
                Ok(target) => {
                    let dir = match &kind {
                        EditKind::NewFile(d) | EditKind::NewDir(d) => d.clone(),
                        EditKind::Rename(p) => {
                            this.forget_paths(p, window, cx);
                            parent(p)
                        }
                    };
                    this.relist(&dir, window, cx);
                    if matches!(kind, EditKind::NewFile(_)) {
                        this.open_file(target, window, cx);
                    }
                }
                Err(e) => this.toast(NotificationType::Error, format!("{e:#}"), window, cx),
            },
        );
        cx.notify();
    }

    /// Delete from the tree: the first press arms (the menu then says Confirm delete).
    pub fn delete_path(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.ide.armed_delete.as_deref() != Some(path.as_str()) {
            self.ide.armed_delete = Some(path);
            cx.notify();
            return;
        }
        self.ide.armed_delete = None;
        let fs = self.fs();
        let p = path.clone();
        self.background(
            window,
            cx,
            move || delete(&fs, &p),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    this.forget_paths(&path, window, cx);
                    this.relist(&parent(&path), window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Delete: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// Tabs of a path that is gone (deleted, renamed) — the path itself or anything under it.
    fn forget_paths(&mut self, gone: &str, window: &mut Window, cx: &mut Context<Self>) {
        let under = format!("{}/", gone.trim_end_matches('/'));
        let tabs: Vec<String> = self
            .ide
            .tabs
            .iter()
            .filter(|t| *t == gone || t.starts_with(&under))
            .cloned()
            .collect();
        for t in tabs {
            // Gone from disk: its edits can't be saved anyway, so no second press.
            self.ide.parked.remove(&t);
            self.ide.armed_close = Some(t.clone());
            self.close_tab(t, window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fuzzy, parent};

    #[test]
    fn fuzzy_prefers_names_and_runs() {
        assert!(fuzzy("cors", "/etc/nginx/cors.conf").is_some());
        assert!(fuzzy("xyz", "/etc/nginx/cors.conf").is_none());
        let name = fuzzy("conf", "/srv/app/site.conf").unwrap();
        let dir = fuzzy("conf", "/srv/config/site.txt").unwrap();
        assert!(name > dir, "{name} vs {dir}");
    }

    #[test]
    fn parents() {
        assert_eq!(parent("/a/b/c.txt"), "/a/b");
        assert_eq!(parent("/a"), "/");
    }
}
