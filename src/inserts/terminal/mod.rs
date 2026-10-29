//! `/terminal`: a terminal in the thread ("ssh:local"), on the machine the thread's project runs
//! on (`thread_machine()`). As tall as its text (its program still sees a whole screen); "to the
//! background" leaves only its head, with a Background badge that brings it back.

pub mod panel;
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
use crate::inserts::state::Insert;
use crate::inserts::InsertKind;
use crate::thread::ThreadRef;
use bench_ui::icons::Ico;

/// The tallest a terminal gets in a thread; its program's screen is this many rows.
const MAX_H: f32 = 280.;

pub struct Terminal;

impl InsertKind for Terminal {
    fn owns(&self, key: &str) -> bool {
        key == "ssh:local"
    }

    fn word(&self) -> crate::inserts::Word {
        crate::inserts::Word {
            word: "terminal",
            aliases: &[],
            desc: "Local terminal",
            key: "ssh:local",
        }
    }

    /// The terminal where the thread's project runs: this Mac's, or its server's.
    fn choices(&self, app: &BenchApp, at: &ThreadRef) -> Vec<crate::thread::composer::slash::SlashCommand> {
        let host = app.thread_host(at);
        let mut row = self.word().command("Tool");
        row.desc = if host.is_empty() {
            "the local terminal".to_string()
        } else {
            format!("the terminal on {host}, where this project runs")
        };
        vec![row]
    }

    fn title(&self, app: &BenchApp, at: &ThreadRef, ins: &Insert) -> String {
        let a = app.thread_machine(at);
        let name = if a.is_empty() {
            "Terminal".to_string()
        } else {
            a
        };
        match app.inserts.ssh.tab_of_insert(&ins.id) {
            Some(t) => app.inserts.ssh.tabs[t]
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
        _at: &ThreadRef,
        ins: &Insert,
        _window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Vec<AnyElement> {
        let Some(t) = app.inserts.ssh.tab_of_insert(&ins.id) else {
            return vec![];
        };
        let theme = cx.theme().clone();
        let id = app.inserts.ssh.tabs[t].id;
        if app.inserts.ssh.tabs[t].background {
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
                    .child(bench_ui::dot(theme.primary))
                    .child("Background")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.click_claimed = true;
                        if let Some(t) = this.inserts.ssh.tabs.iter_mut().find(|t| t.id == id) {
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
                if let Some(t) = this.inserts.ssh.tabs.iter_mut().find(|t| t.id == id) {
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
        at: &ThreadRef,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) -> Option<AnyElement> {
        let alias = app.thread_machine(at);
        if app.demo {
            return Some(
                div()
                    .font_family(bench_ui::theme::MONO)
                    .text_sm()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .children(
                        crate::demo::terminal(&alias)
                            .lines()
                            .map(|l| div().h(px(bench_ui::TERMINAL_LINE_H)).child(l.to_string())),
                    )
                    .into_any_element(),
            );
        }
        let ix = app.insert_terminal(&ins.id, &alias, window, cx)?;
        let tab = &app.inserts.ssh.tabs[ix];
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

    fn context(&self, app: &BenchApp, at: &ThreadRef, ins: &Insert, _cx: &App) -> String {
        let alias = app.thread_machine(at);
        let name = if alias.is_empty() {
            "the local machine".to_string()
        } else {
            alias
        };
        match app.inserts.ssh.tab_of_insert(&ins.id) {
            Some(t) => format!(
                "The terminal of {name} shows (its last lines):\n```\n{}\n```",
                app.inserts.ssh.tabs[t].session.screen_text(60)
            ),
            None => String::new(),
        }
    }

    fn opened(
        &self,
        app: &mut BenchApp,
        at: &ThreadRef,
        ins: &Insert,
        window: &mut Window,
        cx: &mut Context<BenchApp>,
    ) {
        let a = app.thread_machine(at);
        if let Some(t) = app.insert_terminal(&ins.id, &a, window, cx) {
            app.inserts.ssh.tabs[t].background = false;
            app.inserts.ssh.tabs[t].focus.focus(window, cx);
        }
    }

    /// Its shell ends with it.
    fn removed(
        &self,
        app: &mut BenchApp,
        _at: &ThreadRef,
        ins: &Insert,
        _cx: &mut Context<BenchApp>,
    ) {
        if let Some(t) = app.inserts.ssh.tab_of_insert(&ins.id) {
            app.inserts.ssh.tabs[t].session.kill();
            app.inserts.ssh.tabs.remove(t);
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
