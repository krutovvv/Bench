//! The project's terminal in the bottom dock (`Panel::Terminal`, ⌃`, the status bar's button):
//! the same shell as a `/terminal` insert (`servers.rs` `Tab`, `session.rs`), on the project's
//! machine and started in its folder — one per project, kept while Bench runs. Its selection, or
//! its last lines, go to the thread as a reference (`thread/attachments.rs` `TerminalRef`).

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::v_flex;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::thread::attachments::{Reference, TerminalRef};
use crate::workspace::{DockPosition, Panel};
use bench_ui::icons::Ico;

/// How many of its last lines go to the thread when nothing is selected.
const SENT_LINES: usize = 60;

impl BenchApp {
    /// The project the dock's terminal is of: the Explorer's while it is shown, else the open
    /// thread's.
    fn terminal_project(&self) -> String {
        if self.left_dock.shows(Panel::Explorer) && !self.explorer.project.is_empty() {
            self.explorer.project.clone()
        } else {
            self.thread_project_or_host()
        }
    }

    /// Where it runs: the server's name, or "" for this Mac (`session::LOCAL`).
    fn terminal_machine(&self, project: &str) -> String {
        self.location(project).machine_here()
    }

    /// ⌃`: the terminal under the center, or not; shown, it takes the keys.
    pub fn toggle_terminal_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bottom_dock.toggle_panel(Panel::Terminal);
        if self.bottom_dock.shows(Panel::Terminal) {
            if let Some(t) = self.panel_terminal(window, cx) {
                self.ssh.tabs[t].focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// The project's shell (started once, in its folder; again when it has ended).
    fn panel_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        let project = self.terminal_project();
        let key = format!("dock:{project}");
        let fresh = self.ssh.tab_of_insert(&key).is_none_or(|t| {
            self.ssh.tabs[t]
                .session
                .exited
                .load(std::sync::atomic::Ordering::SeqCst)
        });
        let alias = self.terminal_machine(&project);
        let t = self.insert_terminal(&key, &alias, window, cx)?;
        if fresh {
            if let Some(dir) = self.location(&project).folder.filter(|f| !f.is_empty()) {
                let cd = format!(
                    "cd {} && clear\r",
                    crate::inserts::subjects::shell_quote(&dir)
                );
                self.ssh.tabs[t].session.write(cd.as_bytes());
            }
        }
        Some(t)
    }

    /// The selection of the dock's terminal, or its last lines, to the thread's composer.
    pub fn send_terminal_to_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.terminal_project();
        let machine = match self.terminal_machine(&project) {
            m if m.is_empty() => "this Mac".to_string(),
            m => m,
        };
        let text = if self.demo {
            crate::demo::terminal(&machine)
        } else {
            let Some(t) = self.ssh.tab_of_insert(&format!("dock:{project}")) else {
                return;
            };
            let s = &self.ssh.tabs[t].session;
            s.selection_text()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| s.screen_text(SENT_LINES))
        };
        if text.trim().is_empty() {
            self.toast(
                NotificationType::Info,
                "The terminal shows nothing yet",
                window,
                cx,
            );
            return;
        }
        self.add_reference(Reference::Terminal(TerminalRef { machine, text }), cx);
        // The thread to send it in: the center's, or beside a document.
        if self.shown_panel(DockPosition::Right).is_none()
            && self.center != crate::workspace::Center::Chat
        {
            self.right_dock.activate(Panel::Thread);
        }
        self.composer.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    /// The bottom dock's terminal: its head (where it runs; to the thread; close) and the shell
    /// filling the rest.
    pub(crate) fn render_terminal_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let project = self.terminal_project();
        let at = self.location(&project);
        let machine = match at.machine_here() {
            m if m.is_empty() => "this Mac".to_string(),
            m => m,
        };
        let actions = vec![
            bench_ui::form_action("term-send", "Send to the thread", false, cx)
                .on_click(
                    cx.listener(|this, _, window, cx| this.send_terminal_to_thread(window, cx)),
                )
                .into_any_element(),
            bench_ui::head_icon_button("term-close", Ico::X, "Hide  ⌃`", cx)
                .on_click(cx.listener(|this, _, window, cx| this.toggle_terminal_panel(window, cx)))
                .into_any_element(),
        ];
        let title = format!("Terminal · {}", self.project_name(&project));
        let head = bench_ui::ide_head(
            title,
            Some(
                format!("{machine} {}", at.folder.unwrap_or_default())
                    .trim()
                    .to_string()
                    .into(),
            ),
            actions,
            cx,
        );
        let body = if self.demo {
            let alias = machine.clone();
            div()
                .size_full()
                .p_2()
                .bg(cx.theme().secondary)
                .font_family(bench_ui::theme::MONO)
                .text_sm()
                .whitespace_nowrap()
                .overflow_hidden()
                .children(
                    crate::demo::terminal(&alias)
                        .lines()
                        .map(|l| div().child(l.to_string())),
                )
                .into_any_element()
        } else {
            match self.panel_terminal(window, cx) {
                Some(t) => self.render_terminal(t, window, cx),
                None => div().into_any_element(),
            }
        };
        v_flex()
            .size_full()
            .child(head)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}
