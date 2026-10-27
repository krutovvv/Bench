//! ⌘⇧F, as Zed's project search: a field over the tree, the lines found by file, a line opening its
//! file with the caret on it.

use gpui_kit::*;

use super::tree::{act, quiet};
use super::{DocKey, FileRef, Find};
use crate::app::BenchApp;
use crate::client::rest;
use crate::env::enc;
use crate::workspace::Panel;
use bench_ui::icons::Ico;
use gpui_kit::component::input::Input;
use gpui_kit::component::input::{InputEvent, InputState, Position};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};

impl BenchApp {
    /// ⌘⇧F in place of the tree: the field, then the lines found by file; a line opens its file
    /// there.
    pub(super) fn find_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let Some(f) = self.explorer.find.as_ref() else {
            return vec![];
        };
        let close = act(
            "ex-find-close",
            Ico::X,
            "Back to the tree  Esc",
            cx,
            |this, _, cx| {
                this.explorer.find = None;
                cx.notify();
            },
        );
        let mut out = vec![
            h_flex()
                .flex_none()
                .px_2()
                .pb_1()
                .gap_1()
                .items_center()
                .child(div().flex_1().child(Input::new(&f.input).small()))
                .child(close)
                .into_any_element(),
        ];
        let Some(root) = self.explorer_root() else {
            return out;
        };
        match &f.hits {
            None => out.push(quiet("Looking…", 0, cx)),
            Some(h) if h.is_empty() && !f.asked.is_empty() => {
                out.push(quiet("Nothing found", 0, cx))
            }
            Some(hits) => {
                let mut last = String::new();
                for (n, (path, line, text)) in hits.iter().enumerate() {
                    if *path != last {
                        last = path.clone();
                        let (ico, color) = bench_ui::file_icon(path, false, &theme);
                        out.push(
                            h_flex()
                                .flex_none()
                                .px_3()
                                .pt_1()
                                .gap_1p5()
                                .items_center()
                                .text_sm()
                                .child(ico.icon().small().text_color(color))
                                .child(div().min_w_0().truncate().child(path.clone()))
                                .into_any_element(),
                        );
                    }
                    let (file, line) = (root.join(path), *line);
                    out.push(
                        h_flex()
                            .id(("ex-hit", n))
                            .flex_none()
                            .pl(bench_ui::tree_pl(2))
                            .pr_2()
                            .gap_2()
                            .text_xs()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(theme.muted_foreground)
                                    .child(line.to_string()),
                            )
                            .child(div().min_w_0().truncate().child(text.trim().to_string()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_at(file.clone(), line, window, cx)
                            }))
                            .into_any_element(),
                    );
                }
            }
        }
        out
    }
}

impl BenchApp {
    /// ⌘⇧F: the field over the tree.
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.left_dock.shows(Panel::Explorer) {
            self.toggle_side(window, cx);
        }
        if self.explorer_root().is_none() {
            self.toast(
                NotificationType::Info,
                "This project has no folder",
                window,
                cx,
            );
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find in the folder"));
        cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.run_find(window, cx);
            }
        })
        .detach();
        input.update(cx, |s, cx| s.focus(window, cx));
        self.explorer.find = Some(Find {
            input,
            hits: Some(vec![]),
            asked: String::new(),
        });
        cx.notify();
    }

    /// Enter in the find field: the lines that have the text.
    fn run_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(root), Some(f)) = (self.explorer_root(), self.explorer.find.as_mut()) else {
            return;
        };
        let q = f.input.read(cx).value().trim().to_string();
        if q.is_empty() {
            return;
        }
        f.asked = q.clone();
        f.hits = None;
        let Some((conn, _)) = self.env_target(&root.project) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!(
            "/api/fs/grep?path={}&q={}{}",
            enc(&root.path),
            enc(&q),
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
                        if let Some(f) = this.explorer.find.as_mut().filter(|f| f.asked == q) {
                            f.hits = Some(
                                v["hits"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .map(|h| {
                                        (
                                            h["path"].as_str().unwrap_or_default().to_string(),
                                            h["line"].as_u64().unwrap_or(1) as usize,
                                            h["text"].as_str().unwrap_or_default().to_string(),
                                        )
                                    })
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

    /// A found line: its file opens with the caret on it.
    pub fn open_at(
        &mut self,
        f: FileRef,
        line: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_doc(DocKey::File(f.clone()), window, cx);
        if let Some(d) = self.explorer.docs.get_mut(&DocKey::File(f)) {
            d.goto = Some(line.saturating_sub(1) as u32);
            if !d.loading {
                let at = Position::new(d.goto.take().unwrap_or(0), 0);
                d.editor
                    .update(cx, |s, cx| s.set_cursor_position(at, window, cx));
            }
        }
    }
}
