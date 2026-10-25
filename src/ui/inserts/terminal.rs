//! `/terminal`, `/ssh`: a terminal in the thread — this Mac's ("ssh:local") or a server's
//! ("ssh:<alias>"); "ssh" alone lists the servers to pick from. As tall as its text (its program
//! still sees a whole screen); "to the background" leaves its head, and the status bar lists it.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use serde_json::json;

use super::{InsertKind, Switch, SwitchTo, icon_button, pick_row};
use crate::app::BenchApp;
use crate::icons::Ico;
use crate::model::Insert;

/// The tallest a terminal gets in a thread; its program's screen is this many rows.
const MAX_H: f32 = 280.;

pub struct Terminal;

/// The terminal's alias ("" this Mac's), None until one is picked.
fn alias_of(key: &str) -> Option<String> {
    match key.strip_prefix("ssh:")? {
        "local" | "" => Some(crate::term::LOCAL.to_string()),
        a => Some(a.to_string()),
    }
}

/// The servers a thread may open: its project's.
fn servers(app: &BenchApp) -> Vec<(String, String)> {
    let proj = app.thread_project.clone();
    app.ssh
        .servers
        .iter()
        // A project's own servers; outside projects, the servers of no project.
        .filter(|s| app.ssh_projects.get(&s.alias).map(String::as_str).unwrap_or("") == proj)
        .map(|s| (s.alias.clone(), s.hostname.clone()))
        .collect()
}

fn new_server(app: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>) {
    app.edit_server(None, None, window, cx);
}

impl InsertKind for Terminal {
    fn owns(&self, key: &str) -> bool {
        key == "ssh" || key.starts_with("ssh:")
    }

    fn title(&self, app: &BenchApp, ins: &Insert) -> String {
        match alias_of(&ins.key) {
            None => "SSH".into(),
            Some(a) => {
                let name = if a.is_empty() { "Terminal".to_string() } else { a.clone() };
                match app.ssh.tab_of_insert(&ins.id) {
                    Some(t) => app.ssh.tabs[t].session.title().filter(|t| !t.trim().is_empty()).map(|t| format!("{name} · {t}")).unwrap_or(name),
                    None => name,
                }
            }
        }
    }

    fn switches(&self, app: &BenchApp, ins: &Insert) -> Vec<Switch> {
        let now = alias_of(&ins.key);
        // This Mac's terminal is /terminal; /ssh is for servers.
        if now.as_deref() == Some(crate::term::LOCAL) {
            return vec![];
        }
        let mut v = vec![];
        v.extend(servers(app).into_iter().map(|(alias, host)| Switch {
            chosen: now.as_deref() == Some(alias.as_str()),
            to: SwitchTo::Patch(json!({ "key": format!("ssh:{alias}") })),
            label: alias,
            detail: host,
        }));
        v.push(Switch { label: "New server…".into(), detail: String::new(), chosen: false, to: SwitchTo::Run(new_server) });
        v
    }

    fn controls(&self, app: &mut BenchApp, ins: &Insert, _window: &mut Window, cx: &mut Context<BenchApp>) -> Vec<AnyElement> {
        let Some(alias) = alias_of(&ins.key) else {
            return vec![];
        };
        let _ = alias;
        let Some(t) = app.ssh.tab_of_insert(&ins.id) else {
            return vec![];
        };
        let theme = cx.theme().clone();
        let id = app.ssh.tabs[t].id;
        if app.ssh.tabs[t].background {
            return vec![
                h_flex()
                    .id(("term-bg-badge", id))
                    .gap_1()
                    .px_1p5()
                    .border_1().rounded_md()
                    .border_color(theme.border)
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .child(div().size(px(6.)).rounded_full().bg(theme.primary))
                    .child("Background")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.click_claimed = true;
                        if let Some(t) = this.ssh.tabs.iter_mut().find(|t| t.id == id) {
                            t.background = false;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            ];
        }
        vec![
            icon_button(&format!("{}-bg", ins.id), Ico::Terminal, "Send to the background", &theme)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.click_claimed = true;
                    if let Some(t) = this.ssh.tabs.iter_mut().find(|t| t.id == id) {
                        t.background = true;
                    }
                    cx.notify();
                }))
                .into_any_element(),
        ]
    }

    fn body(&self, app: &mut BenchApp, ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) -> Option<AnyElement> {
        let Some(alias) = alias_of(&ins.key) else {
            // Which one: this Mac, a server, or a new one.
            let id = ins.id.clone();
            let mut rows = vec![];
            let pick = |key: String| {
                let id = id.clone();
                move |this: &mut BenchApp, window: &mut Window, cx: &mut Context<BenchApp>| {
                    this.patch_insert(&id, json!({ "key": key }), cx);
                    if let Some(a) = alias_of(&key) {
                        this.insert_terminal(&id, &a, window, cx);
                    }
                }
            };
            for (k, (alias, host)) in servers(app).into_iter().enumerate() {
                rows.push(pick_row(format!("{id}-s{k}"), Ico::Server, alias.clone(), host, cx, pick(format!("ssh:{alias}"))));
            }
            rows.push(pick_row(format!("{id}-new"), Ico::Plus, "New server…".into(), "add one to ~/.ssh/config".into(), cx, new_server));
            return Some(v_flex().children(rows).into_any_element());
        };
        if app.demo {
            return Some(
                div()
                    .font_family(crate::theme::MONO)
                    .text_sm()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .children(crate::demo::terminal(&alias).lines().map(|l| div().h(px(20.)).child(l.to_string())))
                    .into_any_element(),
            );
        }
        let ix = app.insert_terminal(&ins.id, &alias, window, cx)?;
        let tab = &app.ssh.tabs[ix];
        if tab.background {
            return None;
        }
        let line = match tab.session.layout().line_h {
            h if h > 1. => h,
            _ => 20.,
        };
        // Its program sees the whole MAX_H, whatever the view shows.
        tab.session.set_min_rows((((MAX_H - 16.) / line).floor() as usize).max(4));
        let h = super::super::chat::terminal_height(&tab.session).min(MAX_H);
        // The view's own 16 inset pulled back: its text starts where the head's does.
        Some(div().h(px(h)).mx(px(-12.)).overflow_hidden().child(app.render_terminal(ix, window, cx)).into_any_element())
    }

    fn context(&self, app: &BenchApp, ins: &Insert, _cx: &App) -> String {
        let Some(alias) = alias_of(&ins.key) else {
            return String::new();
        };
        let name = if alias.is_empty() { "this Mac".to_string() } else { alias.clone() };
        match app.ssh.tab_of_insert(&ins.id) {
            Some(t) => format!("The terminal of {name} shows (its last lines):\n```\n{}\n```", app.ssh.tabs[t].session.screen_text(60)),
            None => String::new(),
        }
    }

    fn opened(&self, app: &mut BenchApp, ins: &Insert, window: &mut Window, cx: &mut Context<BenchApp>) {
        if let Some(a) = alias_of(&ins.key) {
            if let Some(t) = app.insert_terminal(&ins.id, &a, window, cx) {
                app.ssh.tabs[t].background = false;
                app.ssh.tabs[t].focus.focus(window, cx);
            }
        }
    }

    /// Its shell ends with it.
    fn removed(&self, app: &mut BenchApp, ins: &Insert, _cx: &mut Context<BenchApp>) {
        if let Some(t) = app.ssh.tab_of_insert(&ins.id) {
            app.ssh.tabs[t].session.kill();
            app.ssh.tabs.remove(t);
        }
    }
}
