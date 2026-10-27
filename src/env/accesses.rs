//! The project's accesses, two blocks: Machines — SSH hosts, a mirror of ~/.ssh/config, shared by
//! this Mac (their card only takes one out of the project) — and File hosts — SFTP and FTP, the
//! project's own (their card edits them; /ftp works with them). A remote project has neither
//! here: its accesses are its server's own.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use serde_json::{Value, json};

use super::section::{
    Card, Request, Section, Tab, empty, head_act, pad, remove, revert, row, save,
};
use super::{ProjectEnv, enc, is_remote, on_change};
use crate::app::BenchApp;

/// One access of the project, as its row shows it and its card edits it. A file host is the
/// project's own; an SSH one mirrors ~/.ssh/config.
#[derive(Clone, Debug, Default)]
pub struct Access {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub user: String,
    pub port: Option<u16>,
    pub path: String,
}

impl Access {
    /// What its row says after the kind: "user@host", or the address alone.
    pub fn at(&self) -> String {
        if self.user.is_empty() {
            self.host.clone()
        } else {
            format!("{}@{}", self.user, self.host)
        }
    }
}

pub struct AccessesEnv {
    pub list: Vec<Access>,
    /// The access opened under its row (its id), and the fields being edited. The password is
    /// never read back: an empty field keeps the one that is saved.
    pub open: Option<String>,
    pub name: Entity<InputState>,
    pub host: Entity<InputState>,
    pub port: Entity<InputState>,
    pub user: Entity<InputState>,
    pub password: Entity<InputState>,
    pub path: Entity<InputState>,
    pub dirty: bool,
}

impl AccessesEnv {
    /// Its fields, filled from its row when its card opens.
    pub(super) fn new(window: &mut Window, cx: &mut Context<BenchApp>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name"));
        let host = cx.new(|cx| InputState::new(window, cx).placeholder("example.com"));
        let port = cx.new(|cx| InputState::new(window, cx).placeholder("the usual port"));
        let user = cx.new(|cx| InputState::new(window, cx).placeholder("its user"));
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("empty keeps the saved one")
        });
        let path = cx.new(|cx| {
            InputState::new(window, cx).placeholder("the folder to open in it, /var/www")
        });
        for f in [&name, &host, &port, &user, &password, &path] {
            on_change(f, cx, |e| e.hosts.dirty = true);
        }
        Self {
            list: vec![],
            open: None,
            name,
            host,
            port,
            user,
            password,
            path,
            dirty: false,
        }
    }
}

pub struct Accesses;

impl Section for Accesses {
    fn key(&self) -> &'static str {
        "host"
    }
    fn tab(&self) -> Tab {
        Tab::Access
    }
    fn block(&self, _general: bool) -> &'static str {
        "File hosts"
    }
    fn shown(&self, app: &BenchApp) -> bool {
        app.project_env
            .as_ref()
            .is_some_and(|e| !is_remote(app, &e.id))
    }
    /// "+ Add": the new file host's form under the list.
    fn head(&self, _app: &BenchApp, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        Some(
            head_act("env-host-add".into(), "+ Add", cx)
                .on_click(cx.listener(|this, _, window, cx| this.new_host_form(window, cx)))
                .into_any_element(),
        )
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let mut rows = vec![];
        let list: Vec<Access> = e
            .hosts
            .list
            .iter()
            .filter(|a| a.kind != "ssh")
            .cloned()
            .collect();
        if list.is_empty() {
            rows.push(empty(
                "No file hosts yet: /ftp in its threads offers only these.",
                cx,
            ));
        }
        for (k, a) in list.into_iter().enumerate() {
            let note = format!("{} · {}", a.kind, a.at());
            rows.push(row(e, "host", a.id, a.name, note, false, k == 0, cx));
        }
        rows
    }
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        let e = app.project_env.as_ref()?;
        let muted = cx.theme().muted_foreground;
        let a = e.hosts.list.iter().find(|h| h.id == name).cloned()?;
        let at = a.at();
        if a.kind == "ssh" {
            // An SSH access is the machine's (~/.ssh/config): Bench shows it, but changes to it
            // go in the config, so the card only takes it out of the project.
            let actions = vec![remove(
                e,
                "host",
                a.id.clone(),
                "Take out of the project",
                cx,
            )];
            return Some(Card::new(
                format!("ssh · {}", a.name),
                pad(v_flex().child(div().child(at)).child(
                    div()
                        .text_color(muted)
                        .child("Its address, user and keys live in ~/.ssh/config on this Mac."),
                )),
                actions,
            ));
        }
        // A file host is this project's own: its fields edit it here, and Remove deletes it, its
        // row and its password with it.
        let field = |label: &'static str, f: &Entity<InputState>| {
            h_flex()
                .gap_4()
                .items_center()
                .py_1()
                .child(div().w(px(110.)).flex_none().text_color(muted).child(label))
                .child(div().flex_1().min_w_0().child(Input::new(f)))
                .into_any_element()
        };
        let mut actions = vec![save("host", e.hosts.dirty, cx)];
        actions.extend(revert("host", e.hosts.dirty, cx));
        actions.push(remove(e, "host", a.id.clone(), "Remove", cx));
        let h = &e.hosts;
        Some(Card::new(
            format!("{} · {}", a.kind, a.name),
            pad(v_flex()
                .child(field("Name", &h.name))
                .child(field("Address", &h.host))
                .child(field("Port", &h.port))
                .child(field("User", &h.user))
                .child(field("Password", &h.password))
                .child(field("Folder", &h.path))
                .child(div().pt_2().text_color(muted).child(
                    "This project's own access: its password is saved on its agent; an empty field keeps it.",
                ))),
            actions,
        ))
    }
    fn apply(&self, e: &mut ProjectEnv, v: &Value, _window: &mut Window, _cx: &mut App) {
        e.hosts.list = v["hosts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|h| {
                let s = |k: &str| h[k].as_str().unwrap_or_default().to_string();
                Access {
                    id: s("id"),
                    name: s("name"),
                    kind: s("kind"),
                    host: s("host"),
                    user: s("user"),
                    port: h["port"].as_u64().map(|p| p as u16),
                    path: s("path"),
                }
            })
            .collect();
    }
    fn close(&self, e: &mut ProjectEnv) {
        e.hosts.open = None;
    }
    fn open(&self, app: &mut BenchApp, id: String, w: &mut Window, cx: &mut Context<BenchApp>) {
        app.env_open_host(id, w, cx);
    }
    /// PUT /api/hosts/<id>. The kind is not edited (change it by re-adding); an empty password
    /// keeps the saved one, which is never read back to the client.
    fn save(
        &self,
        app: &mut BenchApp,
        _id: &str,
        _eid: &str,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<Request> {
        let e = app.project_env.as_ref()?;
        let hid = e.hosts.open.clone()?;
        let addr = e.hosts.host.read(cx).value().trim().to_string();
        if addr.is_empty() {
            app.toast(NotificationType::Error, "The address is empty", window, cx);
            return None;
        }
        let empty = |s: &Entity<InputState>, cx: &mut Context<BenchApp>| {
            let v = s.read(cx).value().trim().to_string();
            if v.is_empty() { None } else { Some(v) }
        };
        let h = &e.hosts;
        let mut body = json!({
            "name": h.name.read(cx).value().trim(),
            "kind": h.list.iter().find(|a| a.id == hid).map(|a| a.kind.clone()).unwrap_or_else(|| "ftp".into()),
            "host": addr,
            "port": empty(&h.port, cx).and_then(|p| p.parse::<u16>().ok()),
            "user": h.user.read(cx).value().trim(),
            "path": empty(&h.path, cx),
        });
        if let Some(pw) = empty(&h.password, cx) {
            body["password"] = json!(pw);
        }
        Some(("PUT", format!("/api/hosts/{}", enc(&hid)), body))
    }
    fn saved(&self, e: &mut ProjectEnv) {
        e.hosts.dirty = false;
        // Its card closes: the row shows the new name and address.
        e.hosts.open = None;
        e.card = None;
    }
    /// An access's name and address are shown in /ftp and in its card too.
    fn reload(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
        app.load_remote_hosts(window, cx);
    }
    fn revert(&self, app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) -> bool {
        let Some(e) = app.project_env.as_mut() else {
            return true;
        };
        e.hosts.dirty = false;
        match e.hosts.open.take() {
            Some(hid) => {
                app.env_open_host(hid, window, cx);
                true
            }
            None => false,
        }
    }
    /// A file host is deleted (it is this project's own); an SSH access only leaves the project.
    fn remove_path(&self, eid: &str, name: &str) -> Option<String> {
        Some(format!("/api/projects/{eid}/hosts/{}", enc(name)))
    }
}

impl BenchApp {
    /// An access's row: its fields open under it to edit (again: closes). The saved password is
    /// never read back, so its field starts empty and empty means "keep it".
    pub fn env_open_host(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let h = &mut e.hosts;
        if h.open.as_deref() == Some(&id) && !h.dirty {
            h.open = None;
            cx.notify();
            return;
        }
        let a = h
            .list
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .unwrap_or_default();
        h.name.update(cx, |s, cx| s.set_value(a.name, window, cx));
        h.host.update(cx, |s, cx| s.set_value(a.host, window, cx));
        h.port.update(cx, |s, cx| {
            s.set_value(
                a.port.map(|p| p.to_string()).unwrap_or_default(),
                window,
                cx,
            )
        });
        h.user.update(cx, |s, cx| s.set_value(a.user, window, cx));
        h.password.update(cx, |s, cx| s.set_value("", window, cx));
        h.path.update(cx, |s, cx| s.set_value(a.path, window, cx));
        h.open = Some(id);
        h.dirty = false;
        cx.notify();
    }
}

/// The machines (SSH hosts of ~/.ssh/config) the project's threads may reach (`/ssh`, `ssh_run`).
pub struct Machines;

impl Section for Machines {
    fn key(&self) -> &'static str {
        "ssh-host"
    }
    fn tab(&self) -> Tab {
        Tab::Access
    }
    fn block(&self, _general: bool) -> &'static str {
        "Machines"
    }
    fn shown(&self, app: &BenchApp) -> bool {
        Accesses.shown(app)
    }
    /// "+ Add": the new server's form under the list (it goes into ~/.ssh/config).
    fn head(&self, _app: &BenchApp, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        Some(
            head_act("env-ssh-add".into(), "+ Add", cx)
                .on_click(
                    cx.listener(|this, _, window, cx| this.edit_server(None, None, window, cx)),
                )
                .into_any_element(),
        )
    }
    fn rows(&self, app: &BenchApp, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(e) = app.project_env.as_ref() else {
            return vec![];
        };
        let list: Vec<Access> = e
            .hosts
            .list
            .iter()
            .filter(|a| a.kind == "ssh")
            .cloned()
            .collect();
        if list.is_empty() {
            return vec![empty(
                "No servers yet: /ssh in its threads offers only these.",
                cx,
            )];
        }
        list.into_iter()
            .enumerate()
            .map(|(k, a)| {
                row(
                    e,
                    "ssh-host",
                    a.id.clone(),
                    a.name.clone(),
                    format!("ssh · {}", a.at()),
                    false,
                    k == 0,
                    cx,
                )
            })
            .collect()
    }
    /// The same card as in the accesses: the machine's, shown, taken out of the project.
    fn card(&self, app: &BenchApp, name: &str, cx: &mut Context<BenchApp>) -> Option<Card> {
        Accesses.card(app, name, cx)
    }
}
