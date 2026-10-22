//! The mini IDE: the right panel's file tree + editor, one per window, independent of the mode
//! (Sessions / SSH) — opened from a session's or a server's "⋯", closed from the header.
//!
//! Its files come either through the gateway (this Mac, or a machine running Hermes) or from a
//! server of `~/.ssh/config` over plain ssh. A server's file is edited as a local copy in the
//! cache (`remote::cache_path`); every change of that copy is written back to the server —
//! whether it was saved here or edited by the agent, which gets the copy's path ("Add to chat").

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use gpui_kit::component::diff::DiffFile;

use super::{FileMode, FileView, HermesApp, RightTab, common_dir, diff_state};
use crate::lang::{language_for, language_for_text};
use crate::model::Target;
use crate::{remote, rest};

/// The editor takes files up to this size; a larger one shows its beginning, read-only (the
/// editor stalls on tens of MB), while its local copy stays complete and synced — the agent
/// edits the whole file through its path.
const EDIT_LIMIT: usize = 1_000_000;

/// The beginning of `bytes`, at most `EDIT_LIMIT`, cut at a line end.
fn preview(bytes: &[u8]) -> (String, bool) {
    if bytes.len() <= EDIT_LIMIT {
        return (String::from_utf8_lossy(bytes).to_string(), false);
    }
    let cut = bytes[..EDIT_LIMIT]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(EDIT_LIMIT, |i| i + 1);
    (String::from_utf8_lossy(&bytes[..cut]).to_string(), true)
}

/// A server's file open in the editor.
pub struct RemoteFile {
    pub alias: String,
    pub remote: String,
    pub local: PathBuf,
    /// The content known to be the same here and on the server.
    pub synced: Vec<u8>,
    pub mtime: Option<SystemTime>,
    pub uploading: bool,
    /// The file's modification time on the server when it was read or last written here.
    pub server_mtime: i64,
    /// It changed on the server meanwhile: nothing is written until Reload or Overwrite.
    pub conflict: bool,
}

#[derive(Default)]
pub struct Ide {
    /// The files are a server's (its `~/.ssh/config` alias); None = the gateway's.
    pub server: Option<String>,
    pub remote_file: Option<RemoteFile>,
    /// Watches the open server file's local copy while it is open.
    watch: Option<Task<()>>,
    /// Open files, in tab order (`ide_tabs.rs`); the active one is `HermesApp::file`.
    pub tabs: Vec<String>,
    /// The other tabs as they were left.
    pub parked: std::collections::HashMap<String, Parked>,
    /// The active file's text as saved (to tell unsaved edits), and whether it has some.
    pub baseline: String,
    pub dirty: bool,
    /// A tab whose × was pressed once with unsaved edits; the IDE's close, likewise.
    pub armed_close: Option<String>,
    pub armed_close_all: bool,
    /// ⌘P: find a file under the tree's root (`ide_ops.rs`).
    pub quick: Option<super::ide_ops::Quick>,
    /// A new file/folder or a rename being typed in the tree.
    pub edit: Option<super::ide_ops::TreeEdit>,
    /// A path whose Delete was pressed once.
    pub armed_delete: Option<String>,
    /// The IDE has the whole window (sidebar and chat/terminal hidden, still running).
    pub expanded: bool,
}

/// A tab that is not the active one.
pub struct Parked {
    pub view: FileView,
    pub text: String,
    pub baseline: String,
    pub dirty: bool,
    pub remote: Option<RemoteFile>,
}

fn mtime(p: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl HermesApp {
    pub fn ide_open(&self) -> bool {
        self.right.is_some()
    }

    /// The tree on the gateway's disk: at `root`, else where the open chat works.
    pub fn open_ide(&mut self, root: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.ide.server.is_some() {
            self.close_ide(cx);
        }
        self.right = Some(RightTab::Files);
        match root {
            Some(r) => self.set_tree_root(r, window, cx),
            None => self.load_tree_root(window, cx),
        }
        cx.notify();
    }

    /// The tree of a session's folder: its own folder, else (the open chat) its project or the
    /// folder of the files it touched, else Hermes' working directory.
    pub fn open_ide_for_session(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let root = self.folder_of(&id);
        if root.is_none() && self.session.as_deref() != Some(id.as_str()) {
            self.open_session(id, cx);
        }
        self.open_ide(root, window, cx);
    }

    /// The tree of a server: at its default path (`# path:` in `~/.ssh/config`), else at home.
    pub fn open_ide_server(&mut self, alias: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_ide_server_at(alias, None, window, cx);
    }

    /// The tree of a server at `root` (else its default path, else home).
    pub fn open_ide_server_at(
        &mut self,
        alias: String,
        root: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_ide(cx);
        self.ide.server = Some(alias.clone());
        self.right = Some(RightTab::Files);
        let path = root.unwrap_or_else(|| {
            self.ssh
                .servers
                .iter()
                .find(|s| s.alias == alias)
                .map(|s| s.path.clone())
                .unwrap_or_default()
        });
        if !path.is_empty() {
            self.set_tree_root(path, window, cx);
        } else {
            let a = alias.clone();
            self.background(
                window,
                cx,
                move || remote::home_dir(&a),
                move |this, r, window, cx| match r {
                    Ok(home) if this.ide.server.as_deref() == Some(alias.as_str()) => {
                        this.set_tree_root(home, window, cx)
                    }
                    Ok(_) => {}
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("{alias}: {e:#}"),
                        window,
                        cx,
                    ),
                },
            );
        }
        cx.notify();
    }

    /// Hide the panel and forget what it showed.
    pub fn close_ide(&mut self, cx: &mut Context<Self>) {
        self.right = None;
        self.file = None;
        self.tree_root = None;
        self.tree.clear();
        self.tree_open.clear();
        self.ide = Ide::default();
        cx.notify();
    }

    /// A folder whose only entry is a folder opens that one too: the tree shows them as one
    /// row (`a / b`), as Zed does.
    pub(super) fn open_single_child(
        &mut self,
        dir: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some([(_, child, true)]) = self.tree.get(dir).map(|e| e.as_slice()) {
            let child = child.clone();
            if !self.tree_open.contains(&child) {
                self.open_dir(child, window, cx);
            }
        }
    }

    pub(super) fn list_remote_dir(
        &mut self,
        alias: String,
        dir: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (a, d) = (alias.clone(), dir.clone());
        self.background(
            window,
            cx,
            move || remote::list_dir(&a, &d),
            move |this, r, window, cx| {
                if this.ide.server.as_deref() != Some(alias.as_str()) {
                    return;
                }
                match r {
                    Ok(entries) => {
                        this.tree.insert(dir.clone(), entries);
                        this.open_single_child(&dir, window, cx);
                    }
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("Folder: {e:#}"),
                        window,
                        cx,
                    ),
                }
                cx.notify();
            },
        );
    }

    /// A server's file: copy it into the cache, show the copy, watch it.
    pub(super) fn open_remote_file(
        &mut self,
        alias: String,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (a, p) = (alias.clone(), path.clone());
        self.background(
            window,
            cx,
            move || {
                let (local, server_mtime) = remote::download(&a, &p)?;
                let bytes = std::fs::read(&local)?;
                Ok((local, bytes, server_mtime))
            },
            move |this, r, window, cx| {
                let still = this.file.as_ref().is_some_and(|f| f.path == path)
                    && this.ide.server.as_deref() == Some(alias.as_str());
                if !still {
                    return;
                }
                match r {
                    Ok((local, bytes, server_mtime)) => {
                        let (text, truncated) = preview(&bytes);
                        if let Some(view) = this.file.as_mut() {
                            view.loaded = true;
                            view.truncated = truncated;
                            view.missing = std::str::from_utf8(&bytes).is_err();
                        }
                        let language = crate::lang::language_for_text(&path, &text);
                        this.ide.baseline = text.clone();
                        this.file_editor.update(cx, |s, cx| {
                            s.set_highlighter(language, cx);
                            s.set_value(text, window, cx)
                        });
                        this.ide.remote_file = Some(RemoteFile {
                            alias,
                            remote: path,
                            mtime: mtime(&local),
                            local,
                            synced: bytes,
                            uploading: false,
                            server_mtime,
                            conflict: false,
                        });
                        this.watch_remote_file(window, cx);
                    }
                    Err(e) => {
                        if let Some(view) = this.file.as_mut() {
                            view.missing = true;
                        }
                        this.toast(
                            NotificationType::Error,
                            format!("{alias}: {e:#}"),
                            window,
                            cx,
                        );
                    }
                }
                cx.notify();
            },
        );
    }

    /// Save the editor's text: to the local copy, then to the server.
    pub(super) fn save_remote_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.file_editor.read(cx).value().to_string().into_bytes();
        let Some(rf) = self.ide.remote_file.as_mut() else {
            return;
        };
        if let Err(e) = std::fs::write(&rf.local, &text) {
            self.toast(
                NotificationType::Error,
                format!("Not saved: {e}"),
                window,
                cx,
            );
            return;
        }
        rf.mtime = mtime(&rf.local);
        self.upload_remote(text, true, false, window, cx);
    }

    /// Send `bytes` to the server — unless the file changed there since it was read (`force`
    /// skips that check: Overwrite). A conflict stops all writes until Reload or Overwrite.
    fn upload_remote(
        &mut self,
        bytes: Vec<u8>,
        announce: bool,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(rf) = self.ide.remote_file.as_mut() else {
            return;
        };
        rf.uploading = true;
        let (alias, path) = (rf.alias.clone(), rf.remote.clone());
        let expected = (!force).then_some(rf.server_mtime);
        let sent = bytes.clone();
        cx.spawn_in(window, async move |this, cx| {
            let (a, p) = (alias.clone(), path.clone());
            let r = cx
                .background_spawn(async move { remote::upload_checked(&a, &p, bytes, expected) })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                let rf = this
                    .ide
                    .remote_file
                    .as_mut()
                    .filter(|rf| rf.alias == alias && rf.remote == path);
                let Some(rf) = rf else { return };
                rf.uploading = false;
                match r {
                    Ok(remote::Upload::Done(m)) => {
                        let saved = String::from_utf8_lossy(&sent).to_string();
                        rf.synced = sent;
                        rf.server_mtime = m;
                        rf.conflict = false;
                        this.mark_saved(&path, saved);
                        if announce {
                            this.toast(
                                NotificationType::Success,
                                format!("Saved to {alias}:{path}"),
                                window,
                                cx,
                            );
                        }
                    }
                    Ok(remote::Upload::Conflict) => {
                        rf.conflict = true;
                        this.toast(
                            NotificationType::Warning,
                            format!(
                                "{alias}:{path} changed on the server since it was opened — \
                                 not saved. Reload it, or Overwrite."
                            ),
                            window,
                            cx,
                        );
                    }
                    Err(e) => this.toast(
                        NotificationType::Error,
                        format!("Not saved to {alias}: {e:#}"),
                        window,
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// After a conflict: read the server's version again (local edits are dropped).
    pub fn reload_remote_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.ide.remote_file.as_ref().map(|rf| rf.remote.clone()) {
            self.open_file(path, window, cx);
        }
    }

    /// After a conflict: write this version over the server's.
    pub fn overwrite_remote_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let truncated = self.file.as_ref().is_some_and(|f| f.truncated);
        let Some(local) = self.ide.remote_file.as_ref().map(|rf| rf.local.clone()) else {
            return;
        };
        if !truncated {
            let text = self.file_editor.read(cx).value().to_string();
            let _ = std::fs::write(&local, text);
        }
        if let Some(rf) = self.ide.remote_file.as_mut() {
            rf.mtime = mtime(&local);
        }
        match std::fs::read(&local) {
            Ok(bytes) => self.upload_remote(bytes, true, true, window, cx),
            Err(e) => self.toast(
                NotificationType::Error,
                format!("Not saved: {e}"),
                window,
                cx,
            ),
        }
    }

    /// Once a second while a server file is open: if its local copy changed (the agent edited
    /// it), send it to the server and show it — unless the editor has unsaved edits of its own.
    pub(super) fn watch_remote_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ide.watch = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive =
                    this.update_in(cx, |this, window, cx| this.check_remote_copy(window, cx));
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        }));
    }

    /// Returns false when nothing is watched any more.
    fn check_remote_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(rf) = self.ide.remote_file.as_mut() else {
            return false;
        };
        let now = mtime(&rf.local);
        // In a conflict the copy may change (the agent), but nothing goes out until the user
        // picks Reload or Overwrite.
        if now == rf.mtime || rf.uploading || rf.conflict {
            return true;
        }
        rf.mtime = now;
        let Ok(bytes) = std::fs::read(&rf.local) else {
            return true;
        };
        if bytes == rf.synced {
            return true;
        }
        // A preview (read-only) always follows the file; an editor only without edits of its own.
        let truncated = self.file.as_ref().is_some_and(|f| f.truncated);
        let clean = truncated || {
            let editor = self.file_editor.read(cx).value().to_string().into_bytes();
            editor == rf.synced
        };
        let (alias, remote_path) = (rf.alias.clone(), rf.remote.clone());
        if clean {
            let (text, truncated) = preview(&bytes);
            if let Some(view) = self.file.as_mut() {
                view.truncated = truncated;
            }
            self.ide.baseline = text.clone();
            self.file_editor
                .update(cx, |s, cx| s.set_value(text, window, cx));
        } else {
            self.toast(
                NotificationType::Warning,
                format!(
                    "{alias}:{remote_path} changed on disk; your unsaved edits are kept — Save overwrites"
                ),
                window,
                cx,
            );
        }
        self.upload_remote(bytes, false, false, window, cx);
        cx.notify();
        true
    }

    /// The open file as a chip in the composer (its path; a server file's local copy, which is
    /// synced back), so the agent works on exactly this file.
    pub fn attach_open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let local = match (&self.ide.remote_file, &self.file) {
            (Some(rf), _) => Some(rf.local.clone()),
            (None, Some(f)) if matches!(self.target, crate::model::Target::Local) => {
                Some(PathBuf::from(&f.path))
            }
            _ => None,
        };
        match local {
            Some(p) => {
                self.add_attachments(vec![p], cx);
                // The chip is in the composer: show it — Sessions mode (the IDE stays open),
                // the open chat or a new one, the cursor in the composer.
                if self.ssh.on {
                    self.set_ssh_mode(false, window, cx);
                }
                self.composer.update(cx, |s, cx| s.focus(window, cx));
            }
            None => self.toast(
                NotificationType::Info,
                "Add to chat: only files of this Mac or of a server",
                window,
                cx,
            ),
        }
    }
}

/// The tree and the viewer/editor: roots, folders, opening and saving, Zed.
impl HermesApp {
    /// Switch the right panel's tab (opening it if closed).
    pub fn toggle_right_tab(&mut self, tab: RightTab, window: &mut Window, cx: &mut Context<Self>) {
        self.right = Some(tab);
        if tab == RightTab::Files && self.tree_root.is_none() {
            self.load_tree_root(window, cx);
        }
        cx.notify();
    }

    /// Tree root: the chat's project, else the common folder of the changed files, else the
    /// profile's working dir.
    pub(super) fn load_tree_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let own = self
            .project
            .as_ref()
            .map(|p| p.dir.clone())
            .or_else(|| self.session.as_ref().and_then(|s| self.folder_of(s)));
        if let Some(dir) = own {
            self.set_tree_root(dir, window, cx);
            return;
        }
        let files: Vec<String> = self.session_files().into_iter().map(|(p, _)| p).collect();
        if let Some(root) = common_dir(&files) {
            self.set_tree_root(root, window, cx);
            return;
        }
        self.rest(window, cx, rest::default_cwd, |this, r, window, cx| {
            if let Ok(cwd) = r {
                if !cwd.is_empty() {
                    this.set_tree_root(cwd, window, cx);
                }
            }
        });
    }

    pub fn set_tree_root(&mut self, root: String, window: &mut Window, cx: &mut Context<Self>) {
        self.tree_root = Some(root.clone());
        self.tree.clear();
        self.tree_open.clear();
        self.open_dir(root, window, cx);
    }

    pub fn open_dir(&mut self, dir: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.tree_open.remove(&dir) && self.tree_root.as_deref() != Some(&dir) {
            cx.notify();
            return;
        }
        self.tree_open.insert(dir.clone());
        if self.tree.contains_key(&dir) {
            cx.notify();
            return;
        }
        if let Some(alias) = self.ide.server.clone() {
            self.list_remote_dir(alias, dir, window, cx);
            return;
        }
        let d = dir.clone();
        self.rest(
            window,
            cx,
            move |c, p| rest::list_dir(c, p, &d),
            move |this, r, window, cx| match r {
                Ok(entries) => {
                    this.tree.insert(dir.clone(), entries);
                    this.open_single_child(&dir, window, cx);
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Folder: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// Open a file in the viewer; `diffs` empty means a plain file from the tree.
    pub fn open_file(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        // Another file: the open one becomes a parked tab; a parked one comes back as it was.
        if self.file.as_ref().is_some_and(|f| f.path != path) {
            self.park_current(cx);
        }
        if !self.ide.tabs.contains(&path) {
            self.ide.tabs.push(path.clone());
        }
        self.ide.armed_close = None;
        if self.restore_parked(&path, window, cx) {
            return;
        }
        self.ide.baseline.clear();
        self.ide.dirty = false;
        let diffs = self
            .session_files()
            .into_iter()
            .find(|(p, _)| *p == path)
            .map(|(_, d)| d)
            .unwrap_or_default();
        let mode = if diffs.is_empty() {
            FileMode::After
        } else {
            FileMode::Diff
        };
        self.file = Some(FileView {
            path: path.clone(),
            mode,
            diffs,
            loaded: false,
            truncated: false,
            missing: false,
            diff_view: None,
            before_view: None,
        });
        if let Some(view) = self.file.as_mut() {
            if !view.diffs.is_empty() {
                let files = DiffFile::parse(&crate::diff::joined(&view.diffs)).unwrap_or_default();
                view.diff_view = Some(diff_state(files, cx));
            }
        }
        if self.right.is_none() {
            self.right = Some(RightTab::Changes);
        }
        let language = language_for(&path);
        self.file_editor.update(cx, |s, cx| {
            s.set_highlighter(language, cx);
            s.set_value("", window, cx);
        });
        self.ide.remote_file = None;
        if let Some(alias) = self.ide.server.clone() {
            self.open_remote_file(alias, path, window, cx);
            cx.notify();
            return;
        }
        let p = path.clone();
        self.rest(
            window,
            cx,
            move |c, prof| rest::read_text(c, prof, &p),
            move |this, r, window, cx| {
                let Some(view) = this.file.as_mut().filter(|f| f.path == path) else {
                    return;
                };
                match r {
                    Ok(file) if file.binary => view.missing = true,
                    Ok(file) => {
                        let before = (!view.diffs.is_empty() && !file.truncated)
                            .then(|| crate::diff::before(&file.text, &view.diffs))
                            .flatten();
                        if let Some(before) = before {
                            let full = crate::diff::full(&view.path, &before, &file.text);
                            if let Ok(files) = DiffFile::parse(&full) {
                                if !files.is_empty() {
                                    view.diff_view = Some(diff_state(files, cx));
                                }
                            }
                            let original = DiffFile::unchanged(view.path.clone(), &before);
                            view.before_view = Some(diff_state(vec![original], cx));
                        }
                        view.truncated = file.truncated;
                        view.loaded = true;
                        let language = language_for_text(&view.path, &file.text);
                        this.ide.baseline = file.text.clone();
                        this.file_editor.update(cx, |s, cx| {
                            s.set_highlighter(language, cx);
                            s.set_value(file.text, window, cx)
                        });
                    }
                    // Deleted or moved since (old sessions): no error, just the recorded edits.
                    Err(_) => {
                        view.missing = true;
                        if view.diff_view.is_none() {
                            view.mode = FileMode::After;
                        }
                    }
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    pub fn set_file_mode(&mut self, mode: FileMode, cx: &mut Context<Self>) {
        if let Some(f) = self.file.as_mut() {
            f.mode = mode;
        }
        cx.notify();
    }

    pub fn save_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ide.remote_file.is_some() {
            self.save_remote_file(window, cx);
            return;
        }
        let Some(view) = self.file.as_ref() else {
            return;
        };
        if !view.loaded || view.truncated {
            self.toast(
                NotificationType::Warning,
                "This file was loaded partially — saving is disabled",
                window,
                cx,
            );
            return;
        }
        let path = view.path.clone();
        let text = self.file_editor.read(cx).value().to_string();
        let (saved_path, saved) = (path.clone(), text.clone());
        self.rest(
            window,
            cx,
            move |c, p| rest::write_text(c, p, &path, &text),
            move |this, r, window, cx| match r {
                Ok(()) => {
                    this.mark_saved(&saved_path, saved);
                    this.toast(NotificationType::Success, "Saved", window, cx);
                    cx.notify();
                }
                Err(e) => this.toast(
                    NotificationType::Error,
                    format!("Not saved: {e:#}"),
                    window,
                    cx,
                ),
            },
        );
    }

    /// Open the file in Zed: locally by path, over SSH as ssh://host/path.
    /// The open file in Zed, inside its project when there is one (so Zed opens the whole repo).
    /// The IDE's folder in Zed, with the open file: over SSH for a server (or an SSH gateway),
    /// by path on this Mac.
    pub fn open_in_zed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut paths: Vec<String> = self.tree_root.iter().cloned().collect();
        paths.extend(self.file.as_ref().map(|f| f.path.clone()));
        if paths.is_empty() {
            return;
        }
        match self.ide.server.clone() {
            Some(alias) => {
                let targets = paths.iter().map(|p| format!("ssh://{alias}{p}")).collect();
                self.zed_cli(targets, window, cx);
            }
            None => self.zed(paths, window, cx),
        }
    }

    /// `zed <targets>` (ssh:// targets need the CLI).
    fn zed_cli(&mut self, targets: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let cli = ["/usr/local/bin/zed", "/opt/homebrew/bin/zed"]
            .into_iter()
            .find(|p| std::path::Path::new(p).exists());
        let result = match cli {
            Some(cli) => std::process::Command::new(cli)
                .args(&targets)
                .spawn()
                .map(|_| ()),
            None => Err(std::io::Error::other(
                "the `zed` CLI is not installed (Zed → Install CLI)",
            )),
        };
        if let Err(e) = result {
            self.toast(NotificationType::Error, format!("Zed: {e}"), window, cx);
        }
    }

    fn zed(&mut self, paths: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let targets: Vec<String> = paths
            .into_iter()
            .map(|p| match &self.target {
                Target::Local => p,
                Target::Ssh(host) => format!("ssh://{host}{p}"),
            })
            .collect();
        let zed = ["/usr/local/bin/zed", "/opt/homebrew/bin/zed"]
            .into_iter()
            .find(|p| std::path::Path::new(p).exists());
        let result = match (zed, &self.target) {
            (Some(cli), _) => std::process::Command::new(cli)
                .args(&targets)
                .spawn()
                .map(|_| ()),
            (None, Target::Local) => std::process::Command::new("open")
                .args(["-a", "Zed"])
                .args(&targets)
                .spawn()
                .map(|_| ()),
            (None, Target::Ssh(_)) => Err(std::io::Error::other(
                "the `zed` CLI is not installed (Zed → Install CLI)",
            )),
        };
        if let Err(e) = result {
            self.toast(NotificationType::Error, format!("Zed: {e}"), window, cx);
        }
    }
}
