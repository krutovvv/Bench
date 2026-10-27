//! `/terminal`: a terminal in the thread ("ssh:local"), on the machine the thread's project runs
//! on (`thread_machine()`). As tall as its text (its program still sees a whole screen); "to the
//! background" leaves its head, and the status bar lists it.

pub mod form;
pub mod servers;
#[cfg(not(target_family = "wasm"))]
pub mod session;
#[cfg(target_family = "wasm")]
#[path = "session_web.rs"]
pub mod session;
#[cfg(not(target_family = "wasm"))]
pub mod view;
#[cfg(target_family = "wasm")]
#[path = "view_web.rs"]
pub mod view;

use gpui_kit::component::{ActiveTheme as _, h_flex};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::icons::Ico;
use crate::inserts::InsertKind;
use crate::model::Insert;

/// The tallest a terminal gets in a thread; its program's screen is this many rows.
const MAX_H: f32 = 280.;

pub struct Terminal;

impl InsertKind for Terminal {
    fn owns(&self, key: &str) -> bool {
        key == "ssh:local"
    }

    fn title(&self, app: &BenchApp, ins: &Insert) -> String {
        let a = app.thread_machine();
        let name = if a.is_empty() {
            "Terminal".to_string()
        } else {
            a
        };
        match app.ssh.tab_of_insert(&ins.id) {
            Some(t) => app.ssh.tabs[t]
                .session
                .title()
                .filter(|t| !t.trim().is_empty())
                .map(|t| format!("{name} · {t}"))
                .unwrap_or(name),
            None => name,
        }
    }

    fn controls(
        &self,
        app: &mut BenchApp,
        ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Vec<AnyElement> {
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
                    .border_1()
                    .rounded_md()
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
            bench_ui::head_icon_button(
                &format!("{}-bg", ins.id),
                Ico::Terminal,
                "Send to the background",
                cx,
            )
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

    fn body(
        &self,
        app: &mut BenchApp,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let alias = app.thread_machine();
        if app.demo {
            return Some(
                div()
                    .font_family(crate::theme::MONO)
                    .text_sm()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .children(
                        crate::demo::terminal(&alias)
                            .lines()
                            .map(|l| div().h(px(20.)).child(l.to_string())),
                    )
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
        tab.session
            .set_min_rows((((MAX_H - 16.) / line).floor() as usize).max(4));
        let h = terminal_height(&tab.session).min(MAX_H);
        // The view's own 16 inset pulled back: its text starts where the head's does.
        Some(
            div()
                .h(px(h))
                .mx(px(-12.))
                .overflow_hidden()
                .child(app.render_terminal(ix, window, cx))
                .into_any_element(),
        )
    }

    fn context(&self, app: &BenchApp, ins: &Insert, _cx: &App) -> String {
        let alias = app.thread_machine();
        let name = if alias.is_empty() {
            "this Mac".to_string()
        } else {
            alias
        };
        match app.ssh.tab_of_insert(&ins.id) {
            Some(t) => format!(
                "The terminal of {name} shows (its last lines):\n```\n{}\n```",
                app.ssh.tabs[t].session.screen_text(60)
            ),
            None => String::new(),
        }
    }

    fn opened(
        &self,
        app: &mut BenchApp,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        let a = app.thread_machine();
        if let Some(t) = app.insert_terminal(&ins.id, &a, window, cx) {
            app.ssh.tabs[t].background = false;
            app.ssh.tabs[t].focus.focus(window, cx);
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

/// A terminal as tall as the lines it shows (the cursor's included), with its 16 px padding.
fn terminal_height(session: &crate::inserts::terminal::session::Session) -> f32 {
    let line = match session.layout().line_h {
        h if h > 1. => h,
        _ => 20.,
    };
    session.used_rows().min(1000).max(1) as f32 * line + 16.
}
