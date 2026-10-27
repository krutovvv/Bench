//! ⌘P, as Zed's file finder: a field over the middle and every file of the project's folder to
//! choose from, matched by its letters in order.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;

use super::{DocKey, Quick};
use crate::app::BenchApp;
use crate::workspace::SidebarPanel;
use crate::env::enc;
use crate::client::rest;

impl BenchApp {
    /// ⌘P: a field over the middle and every file of the project's folder to choose from.
    pub fn open_quick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_panel != SidebarPanel::Explorer || self.explorer.project.is_empty() {
            let project = self.thread_project_or_host();
            self.show_explorer(project, window, cx);
        }
        let Some(root) = self.explorer_root() else {
            self.toast(
                NotificationType::Info,
                "This project has no folder",
                window,
                cx,
            );
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("A file's name"));
        cx.subscribe_in(
            &input,
            window,
            |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => this.quick_open_chosen(window, cx),
                InputEvent::Change => {
                    if let Some(q) = this.explorer.quick.as_mut() {
                        q.sel = 0;
                    }
                    cx.notify();
                }
                _ => {}
            },
        )
        .detach();
        input.update(cx, |s, cx| s.focus(window, cx));
        self.explorer.quick = Some(Quick {
            input,
            files: None,
            sel: 0,
        });
        let Some((conn, _)) = self.env_target(&root.project) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!(
            "/api/fs/files?path={}{}",
            enc(&root.path),
            root.machine
                .as_deref()
                .map(|m| format!("&machine={}", enc(m)))
                .unwrap_or_default()
        );
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| {
                match r {
                    Ok(v) => {
                        if let Some(q) = this.explorer.quick.as_mut() {
                            q.files = Some(
                                v["files"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(|f| f.as_str().map(str::to_string))
                                    .collect(),
                            );
                        }
                    }
                    Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    /// ⌘P's choices for what is typed: the files whose path has its letters in order, the name's
    /// matches first, at most 50.
    pub fn quick_matches(&self, cx: &App) -> Vec<String> {
        let Some(q) = self.explorer.quick.as_ref() else {
            return vec![];
        };
        let Some(files) = q.files.as_ref() else {
            return vec![];
        };
        let typed = q.input.read(cx).value().to_lowercase();
        let mut scored: Vec<(usize, &String)> = files
            .iter()
            .filter_map(|f| fuzzy(&f.to_lowercase(), &typed).map(|s| (s, f)))
            .collect();
        scored.sort_by_key(|(s, f)| (*s, f.len()));
        scored
            .into_iter()
            .take(50)
            .map(|(_, f)| f.clone())
            .collect()
    }

    /// ⌘P's ↑ ↓.
    pub fn quick_move(&mut self, by: isize, cx: &mut Context<Self>) {
        let n = self.quick_matches(cx).len();
        if let Some(q) = self.explorer.quick.as_mut() {
            if n > 0 {
                q.sel = (q.sel as isize + by).rem_euclid(n as isize) as usize;
            }
        }
        cx.notify();
    }

    /// ⌘P's Enter: the chosen file opens.
    pub fn quick_open_chosen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sel = self.explorer.quick.as_ref().map_or(0, |q| q.sel);
        let Some(rel) = self.quick_matches(cx).into_iter().nth(sel) else {
            return;
        };
        self.explorer.quick = None;
        if let Some(root) = self.explorer_root() {
            self.open_doc(DocKey::File(root.join(&rel)), window, cx);
        }
    }
}

/// Whether `typed`'s letters are in `path` in order; how good the match is (lower is better: the
/// letters close together and in the file's name).
fn fuzzy(path: &str, typed: &str) -> Option<usize> {
    if typed.is_empty() {
        return Some(0);
    }
    let name_at = path.rfind('/').map_or(0, |i| i + 1);
    let mut chars = path.char_indices();
    let mut first = None;
    let mut last = 0;
    for t in typed.chars().filter(|c| !c.is_whitespace()) {
        let (i, _) = chars.by_ref().find(|(_, c)| *c == t)?;
        first.get_or_insert(i);
        last = i;
    }
    let spread = last - first.unwrap_or(0);
    Some(
        spread
            + if first.unwrap_or(0) >= name_at {
                0
            } else {
                1000
            },
    )
}
