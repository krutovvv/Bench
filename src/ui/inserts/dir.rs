//! `/files`: the project's own folder (on this Mac, or on a remote project's server), a small
//! file manager — folders open in place, a file opens under the insert at its full height with
//! its highlighting, Save writes it back. Its state: {path, file}.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::{Map, json};

use super::{InsertKind, clip, icon_button};
use crate::app::BenchApp;
use crate::icons::Ico;
use crate::model::Insert;

pub struct Dir;

/// The parent of a folder ("/" at the top).
fn parent(path: &str) -> String {
    match path.trim_end_matches('/').rsplit_once('/') {
        Some(("", _)) | None => "/".into(),
        Some((p, _)) => p.to_string(),
    }
}

impl InsertKind for Dir {
    fn owns(&self, key: &str) -> bool {
        key == "dir"
    }

    fn shared(&self) -> Option<&'static str> {
        Some("dir")
    }

    fn title(&self, app: &BenchApp, _ins: &Insert) -> String {
        let f = &app.local_files;
        let name = f.path.trim_end_matches('/').rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("Files");
        format!("Files · {name}")
    }

    fn controls(&self, app: &mut BenchApp, _ins: &Insert, _window: &mut Window, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let path = app.local_files.path.clone();
        if path.is_empty() {
            return vec![];
        }
        vec![div().min_w_0().truncate().text_sm().text_color(theme.muted_foreground).child(path).into_any_element()]
    }

    fn body(&self, app: &mut BenchApp, ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        if app.local_files.owner.as_deref() != Some(ins.id.as_str()) {
            // Its own place again (the thread reopened, or another insert showed another folder).
            app.local_files.owner = Some(ins.id.clone());
            app.local_files.file = None;
            let path = Some(ins.get("path")).filter(|p| !p.is_empty()).unwrap_or_else(|| app.project_folder());
            let path = if path.is_empty() { "~".to_string() } else { path };
            app.show_dir(path, ins.get("file"), window, cx);
        } else {
            let f = &app.local_files;
            if !f.loading && f.error.is_none() && !f.path.is_empty() {
                let mut state = Map::new();
                state.insert("path".into(), json!(f.path));
                state.insert("file".into(), json!(f.file.clone().unwrap_or_default()));
                app.sync_insert_state(&ins.id, state, cx);
            }
        }
        let f = &app.local_files;
        if let Some(e) = &f.error {
            return Some(div().text_base().text_color(theme.danger).child(e.clone()).into_any_element());
        }
        if f.entries.is_empty() {
            return Some(div().text_base().text_color(muted).child(if f.loading { "Reading the folder…" } else { "Empty folder" }).into_any_element());
        }
        let open = f.file.clone();
        // As long as the folder is: the thread scrolls, not the list.
        // ".." first, as in a file manager: up a folder.
        let up = (f.path != "/").then(|| (String::from(".."), parent(&f.path), true));
        let rows: Vec<AnyElement> = up
            .into_iter()
            .chain(f.entries.clone())
            .enumerate()
            .map(|(k, (name, path, is_dir))| {
                let chosen = open.as_deref() == Some(path.as_str());
                h_flex()
                    .id((SharedString::from(format!("{}-row", ins.id)), k))
                    .px_3()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .text_base()
                    .cursor_pointer()
                    .when(k > 0, |d| d.border_t_1().border_color(theme.border.opacity(0.5)))
                    .when(chosen, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(if is_dir { Ico::Folder } else { Ico::File }.icon().small().text_color(muted))
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.open_local_entry(path.clone(), is_dir, window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        Some(v_flex().mx(px(-12.)).my(px(-8.)).children(rows).into_any_element())
    }

    /// The open file, as the next row of the thread.
    fn follow(&self, app: &mut BenchApp, ins: &Insert, _window: &mut Window, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        let f = &app.local_files;
        if f.owner.as_deref() != Some(ins.id.as_str()) {
            return None;
        }
        let path = f.file.clone()?;
        let theme = cx.theme().clone();
        let dirty = f.dirty(cx);
        let lines = f.editor.read(cx).value().lines().count().max(1);
        let head = super::super::block_head(&theme, true)
            .child(Ico::File.icon().small())
            .child(div().flex_1().min_w_0().truncate().text_color(theme.foreground).child(path.clone()))
            .when(f.truncated, |d| d.child("too big: shown in part, not saved"))
            .child(
                Button::new(SharedString::from(format!("{}-save", ins.id)))
                    .small()
                    .map(|b| if dirty { b.primary() } else { b.ghost() })
                    .label("Save")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.click_claimed = true;
                        this.save_local_file(window, cx)
                    })),
            )
            .child(icon_button(&format!("{}-zed", ins.id), Ico::Zed, "Open in Zed", &theme).on_click(cx.listener(move |this, _, _, cx| {
                this.click_claimed = true;
                let host = this.thread_host();
                this.open_in_zed(&path, None, Some(host.as_str()), cx)
            })))
            .child(icon_button(&format!("{}-close", ins.id), Ico::X, "Close the file", &theme).on_click(cx.listener(|this, _, _, cx| {
                this.click_claimed = true;
                if !this.local_files.dirty(cx) {
                    this.local_files.file = None;
                }
                cx.notify();
            })));
        let body = div()
            .h(px(lines as f32 * super::super::editor_line(cx) + 12.))
            .child(gpui_kit::component::input::Editor::new(&app.local_files.editor).appearance(false).h_full())
            .into_any_element();
        Some(super::card(head, body, &theme))
    }

    fn context(&self, app: &BenchApp, ins: &Insert, cx: &App) -> String {
        let f = &app.local_files;
        if f.owner.as_deref() != Some(ins.id.as_str()) {
            return String::new();
        }
        let names: Vec<String> = f.entries.iter().take(80).map(|e| if e.2 { format!("{}/", e.0) } else { e.0.clone() }).collect();
        let mut out = format!("The files insert is in the folder {}. It lists: {}", f.path, names.join(", "));
        if let Some(file) = &f.file {
            out.push_str(&format!(
                "\nOpen under it: {file}{}\n```\n{}\n```",
                if f.dirty(cx) { " (edited, not saved)" } else { "" },
                clip(&f.editor.read(cx).value(), 6000)
            ));
        }
        out
    }
}
