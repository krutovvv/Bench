//! `/ftp`: a file host in the thread, a small Transmit — the hosts to pick from (and a new one),
//! then the host's folders and files, a file open in an editor with its highlighting. Its state:
//! {host, path, file}; the thread shows them again when it is reopened.

use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::*;
use serde_json::{Map, json};

use super::{InsertKind, Switch, SwitchTo, clip, pick_row};
use crate::app::BenchApp;
use crate::icons::Ico;
use crate::model::Insert;

pub struct Files;

/// The hosts a thread may open: its project's.
fn hosts(app: &BenchApp) -> Vec<(String, String, String)> {
    let proj = app.thread_project.clone();
    app.files_remote
        .hosts
        .iter()
        // A project's own hosts; outside projects, the hosts of no project.
        .filter(|h| h.project == proj)
        .map(|h| (h.id.clone(), h.name.clone(), format!("{} · {}@{}", h.kind.to_uppercase(), h.user, h.host)))
        .collect()
}

fn new_host(app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    app.new_host_form(window, cx);
}

impl InsertKind for Files {
    fn owns(&self, key: &str) -> bool {
        key == "files"
    }

    fn shared(&self) -> Option<&'static str> {
        Some("files")
    }

    fn title(&self, app: &BenchApp, ins: &Insert) -> String {
        let host = ins.get("host");
        app.files_remote
            .hosts
            .iter()
            .find(|h| h.id == host)
            .map(|h| format!("{} · {}", h.name, h.kind.to_uppercase()))
            .unwrap_or_else(|| "Files".into())
    }

    fn switches(&self, app: &BenchApp, ins: &Insert) -> Vec<Switch> {
        let now = ins.get("host");
        let mut v: Vec<Switch> = hosts(app)
            .into_iter()
            .map(|(id, name, detail)| Switch {
                chosen: id == now,
                to: SwitchTo::Patch(json!({ "state": { "host": id, "path": "", "file": "" } })),
                label: name,
                detail,
            })
            .collect();
        v.push(Switch { label: "New host…".into(), detail: "FTP, FTPS or SFTP".into(), chosen: false, to: SwitchTo::Run(new_host) });
        v
    }

    fn controls(&self, app: &mut BenchApp, ins: &Insert, _window: &mut Window, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let f = &app.files_remote;
        if f.owner.as_deref() != Some(ins.id.as_str()) || f.path.is_empty() {
            return vec![];
        }
        let mut v = vec![div().min_w_0().truncate().text_sm().text_color(cx.theme().muted_foreground).child(f.path.clone()).into_any_element()];
        v.extend(super::file_buttons(&ins.id, true, cx));
        v
    }

    fn body(&self, app: &mut BenchApp, ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        let host = ins.get("host");
        if host.is_empty() {
            let id = ins.id.clone();
            let mut rows = vec![];
            for (k, (hid, name, detail)) in hosts(app).into_iter().enumerate() {
                let id = id.clone();
                let armed = app.confirm_delete.as_deref() == Some(format!("host:{hid}").as_str());
                let gone = hid.clone();
                rows.push(super::pick_row_removable(format!("{id}-h{k}"), Ico::Server, name, detail, armed, cx, move |this, _, cx| {
                    this.patch_insert(&id, json!({ "state": { "host": hid, "path": "", "file": "" } }), cx);
                }, move |this, window, cx| this.remove_file_host(gone.clone(), window, cx)));
            }
            let from = id.clone();
            rows.push(pick_row(format!("{id}-new"), Ico::Plus, "Add a host…".into(), "FTP, FTPS or SFTP".into(), cx, move |this, window, cx| {
                this.form_after = Some(from.clone());
                new_host(this, window, cx)
            }));
            return Some(v_flex().children(rows).into_any_element());
        }
        let theme = cx.theme().clone();
        if app.files_remote.owner.as_deref() != Some(ins.id.as_str()) {
            // Another insert has edits not saved: they are not thrown away by showing this one.
            if app.files_remote.dirty {
                let file = app.files_remote.file.clone().unwrap_or_default();
                return Some(
                    div()
                        .py_1()
                        .text_base()
                        .text_color(theme.muted_foreground)
                        .child(format!("{file} has edits not saved in another thread: save them there first."))
                        .into_any_element(),
                );
            }
            app.files_remote.owner = Some(ins.id.clone());
            app.show_files_at(&host, &ins.get("path"), &ins.get("file"), window, cx);
        } else {
            // What it shows now is its state.
            let f = &app.files_remote;
            if f.host.as_deref() == Some(host.as_str()) && !f.loading && f.error.is_none() && !f.path.is_empty() {
                let mut state = Map::new();
                state.insert("host".into(), json!(host));
                state.insert("path".into(), json!(f.path));
                state.insert("file".into(), json!(f.file.clone().unwrap_or_default()));
                app.sync_insert_state(&ins.id, state, cx);
            }
        }
        Some(app.render_files_block(&host, false, window, cx))
    }

    /// The open file, as the next row of the thread.
    fn follow(&self, app: &mut BenchApp, ins: &Insert, _window: &mut Window, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        if app.files_remote.owner.as_deref() != Some(ins.id.as_str()) {
            return None;
        }
        // A new file or folder being named, then the open file.
        let form = super::file_form(app, &ins.id, cx);
        let file = app.render_open_file(format!("{}-file-close", ins.id), cx);
        match (form, file) {
            (None, None) => None,
            (a, b) => Some(gpui_kit::component::v_flex().w_full().gap_4().children(a).children(b).into_any_element()),
        }
    }

    fn context(&self, app: &BenchApp, ins: &Insert, cx: &App) -> String {
        let f = &app.files_remote;
        let host = ins.get("host");
        if host.is_empty() {
            return "The file host insert has no host chosen yet.".into();
        }
        if f.owner.as_deref() != Some(ins.id.as_str()) || f.host.as_deref() != Some(host.as_str()) {
            return String::new();
        }
        let names: Vec<String> = f.entries.iter().take(80).map(|e| if e.dir { format!("{}/", e.name) } else { e.name.clone() }).collect();
        let mut out = format!("The file host insert (host id {host}) is in the folder {}. It lists: {}", f.path, names.join(", "));
        if let Some(file) = &f.file {
            out.push_str(&format!(
                "\nOpen in its editor: {file}{}\n```\n{}\n```",
                if f.dirty { " (edited, not saved)" } else { "" },
                clip(&f.editor.read(cx).value(), 6000)
            ));
        }
        out
    }

    fn opened(&self, app: &mut BenchApp, _ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) {
        if app.files_remote.hosts.is_empty() {
            app.load_remote_hosts(window, cx);
        }
    }
}
