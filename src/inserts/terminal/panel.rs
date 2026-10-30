//! The bottom dock's terminals (`Panel::Terminal`, ⌃`, the Code status bar's button): the same shells
//! as a `/terminal` insert (`state.rs` `Tab`, `session.rs`), each on its project's machine and
//! started in its folder. Tabs appear when there is more than one (⌘T adds one for the shown
//! project); they live while Bench runs. The active one's selection, or its last lines, go to
//! the thread as a reference (`thread/composer/attachments.rs` `TerminalRef`).

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::h_flex;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::v_flex;
use gpui_kit::*;
use bench_ui::Gap;

use crate::app::BenchApp;
use crate::thread::composer::attachments::{Reference, TerminalRef};
use crate::workspace::{DockPosition, Panel};
use bench_ui::icons::Ico;

/// How many of its last lines go to the thread when nothing is selected.
const SENT_LINES: usize = 60;

/// One terminal of the bottom dock: its shell's key in `ssh.tabs` and the project it runs in
/// (its machine and folder follow from there, `places.rs`).
#[derive(Clone, Debug)]
pub struct DockTerm {
    pub key: String,
    pub project: String,
}

/// The bottom dock's terminals: each a shell of its own, the shown one fills the panel.
#[derive(Default)]
pub struct DockTerms {
    pub items: Vec<DockTerm>,
    pub active: usize,
    next: usize,
}

impl DockTerms {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn active_term(&self) -> Option<&DockTerm> {
        self.items.get(self.active)
    }

    /// A new terminal of `project`, shown at once; its key.
    pub fn add(&mut self, project: &str) -> String {
        self.next += 1;
        let key = format!("dock:{}", self.next);
        self.items.push(DockTerm {
            key: key.clone(),
            project: project.to_string(),
        });
        self.active = self.items.len() - 1;
        key
    }

    /// Close tab `i`: its caller kills the shell behind `key`; the shown slot moves to a
    /// neighbour.
    pub fn remove(&mut self, i: usize) -> Option<DockTerm> {
        if i >= self.items.len() {
            return None;
        }
        let t = self.items.remove(i);
        self.active = self.active.min(self.items.len().saturating_sub(1));
        Some(t)
    }
}

impl BenchApp {
    /// The project a new dock terminal takes: the Explorer's while it is shown, else the open
    /// thread's. Existing tabs keep the project they were born with.
    pub(crate) fn terminal_project(&self, cx: &App) -> String {
        if self.left_dock.shows(Panel::Explorer) && !self.explorer.project.is_empty() {
            self.explorer.project.clone()
        } else {
            self.thread_ref(cx).project_or_host()
        }
    }

    /// Where a dock terminal runs: the server's name, or "" for this Mac (`session::LOCAL`).
    fn terminal_machine(&self, project: &str) -> String {
        self.location(project).machine_here()
    }

    /// ⌃`: the terminals under the center, or not; shown, the active one takes the keys.
    pub fn toggle_terminal_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bottom_dock.toggle_panel(Panel::Terminal);
        if self.bottom_dock.shows(Panel::Terminal) {
            if self.inserts.dock_terms.is_empty() {
                let project = self.terminal_project(cx);
                self.inserts.dock_terms.add(&project);
            }
            if let Some(t) = self.dock_active_shell(window, cx) {
                self.inserts.terminal.tabs[t].focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// ⌘T and the head's +: another terminal of the shown project, started and shown at once.
    pub fn dock_term_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.terminal_project(cx);
        self.inserts.dock_terms.add(&project);
        self.bottom_dock.activate(Panel::Terminal);
        if let Some(t) = self.dock_active_shell(window, cx) {
            self.inserts.terminal.tabs[t].focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Show tab `i`; its shell starts anew if it had ended.
    pub fn dock_term_show(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        if i >= self.inserts.dock_terms.len() || i == self.inserts.dock_terms.active {
            return;
        }
        self.inserts.dock_terms.active = i;
        if let Some(t) = self.dock_active_shell(window, cx) {
            self.inserts.terminal.tabs[t].focus.focus(window, cx);
        }
        cx.notify();
    }

    /// A tab's ×: its shell dies with it; closing the last one folds the dock.
    pub fn dock_term_close(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.inserts.dock_terms.remove(i) else {
            return;
        };
        if let Some(ix) = self.inserts.terminal.tab_of_insert(&t.key) {
            self.inserts.terminal.tabs[ix].session.kill();
            self.inserts.terminal.tabs.remove(ix);
        }
        if self.inserts.dock_terms.is_empty() {
            self.bottom_dock.open = false;
        } else if self.bottom_dock.shows(Panel::Terminal) {
            if let Some(t) = self.dock_active_shell(window, cx) {
                self.inserts.terminal.tabs[t].focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// The shown terminal's shell, started if it is new or its shell had ended (then cd'd into
    /// the project's folder once, as a new login shell would be).
    fn dock_active_shell(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        let term = self.inserts.dock_terms.active_term()?.clone();
        self.dock_shell(&term, window, cx)
    }

    fn dock_shell(
        &mut self,
        term: &DockTerm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let fresh = self.inserts.terminal.tab_of_insert(&term.key).is_none_or(|t| {
            self.inserts.terminal.tabs[t]
                .session
                .exited
                .load(std::sync::atomic::Ordering::SeqCst)
        });
        let alias = self.terminal_machine(&term.project);
        let t = self.insert_terminal(&term.key, &alias, window, cx)?;
        if fresh {
            if let Some(dir) = self
                .location(&term.project)
                .folder
                .filter(|f| !f.is_empty())
            {
                let cd = format!(
                    "cd {} && clear\r",
                    crate::inserts::slash::shell_quote(&dir)
                );
                self.inserts.terminal.tabs[t].session.write(cd.as_bytes());
            }
        }
        Some(t)
    }

    /// The selection of the shown terminal, or its last lines, to the thread's composer.
    pub fn send_terminal_to_thread(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(term) = self.inserts.dock_terms.active_term().cloned() else {
            return;
        };
        let machine = match self.terminal_machine(&term.project) {
            m if m.is_empty() => "local".to_string(),
            m => m,
        };
        let Some(t) = self.inserts.terminal.tab_of_insert(&term.key) else {
            return;
        };
        let s = &self.inserts.terminal.tabs[t].session;
        let text = s
            .selection_text()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| s.screen_text(SENT_LINES));
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
        self.focus_composer(window, cx);
        cx.notify();
    }

    /// The tab's label: what its shell calls itself, else its project; numbered among tabs of
    /// the same project. `ended` says its shell is gone.
    fn dock_term_label(&self, i: usize) -> (String, bool) {
        let t = &self.inserts.dock_terms.items[i];
        let same = self.inserts.dock_terms.items[..i]
            .iter()
            .filter(|o| o.project == t.project)
            .count();
        let name = match self.inserts.terminal.tab_of_insert(&t.key) {
            Some(ix) => self.inserts.terminal.tabs[ix]
                .session
                .title()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| self.project_name(&t.project)),
            None => self.project_name(&t.project),
        };
        let ended = self.inserts.terminal.tab_of_insert(&t.key).is_some_and(|ix| {
            self.inserts.terminal.tabs[ix]
                .session
                .exited
                .load(std::sync::atomic::Ordering::SeqCst)
        });
        let label = if self
            .inserts.dock_terms
            .items
            .iter()
            .filter(|o| o.project == t.project)
            .count()
            > 1
        {
            format!("{name} · {}", same + 1)
        } else {
            name
        };
        (label, ended)
    }

    /// The bottom dock's terminals: the tabs when there are several, the head (where the shown
    /// one runs; new; to the thread; hide) and the shell filling the rest.
    pub(crate) fn render_terminal_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(active) = self.inserts.dock_terms.active_term().cloned() else {
            return div().into_any_element();
        };
        let at = self.location(&active.project);
        let machine = match at.machine_here() {
            m if m.is_empty() => "local".to_string(),
            m => m,
        };
        let note = format!("{machine} · {}", at.folder.unwrap_or_default())
            .trim()
            .to_string();
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
        let plus = bench_ui::head_icon_button("term-new", Ico::Plus, "New terminal  ⌘T", cx)
            .on_click(cx.listener(|this, _, window, cx| this.dock_term_add(window, cx)))
            .into_any_element();
        let theme = cx.theme().clone();
        let head = if self.inserts.dock_terms.len() <= 1 {
            let mut acts = vec![plus];
            acts.extend(actions);
            bench_ui::ide_head(
                format!("Terminal · {}", self.project_name(&active.project)),
                Some(note.into()),
                acts,
                cx,
            )
        } else {
            let tabs: Vec<AnyElement> = (0..self.inserts.dock_terms.len())
                .map(|i| {
                    let (label, ended) = self.dock_term_label(i);
                    let close = bench_ui::head_icon_button(
                        &format!("term-tab-x-{i}"),
                        Ico::X,
                        "Close terminal",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.dock_term_close(i, window, cx)
                    }));
                    bench_ui::term_tab(
                        format!("term-tab-{i}"),
                        label,
                        i == self.inserts.dock_terms.active,
                        ended,
                        close,
                        cx,
                    )
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(if ended {
                            "Its shell ended — click to start it anew"
                        } else {
                            "Show this terminal"
                        })
                        .build(window, cx)
                    })
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.dock_term_show(i, window, cx)),
                    )
                    .into_any_element()
                })
                .collect();
            h_flex()
                .flex_none()
                .h(px(bench_ui::metrics::BLOCK_HEAD_H))
                .px(Gap::M)
                .gap(Gap::M)
                .items_center()
                // No fill: the bar is the editor's ground with a rule under it, as Xcode's
                // debug area; the terminals a segmented control, as Chat | Code.
                .border_b_1()
                .border_color(theme.border)
                .text_size(px(bench_ui::BAR_TEXT))
                .child(bench_ui::segment_track(&theme).children(tabs))
                .child(plus)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(bench_ui::faint(&theme))
                        .child(note.clone()),
                )
                .children(actions)
        };
        let body = match self.dock_active_shell(window, cx) {
            Some(t) => self.render_terminal(t, window, cx),
            None => div().into_any_element(),
        };
        v_flex()
            .size_full()
            .child(head)
            .child(div().flex_1().min_h_0().child(body))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::DockTerms;

    #[test]
    fn tabs_live_their_own_lives_and_the_shown_slot_moves() {
        let mut d = DockTerms::default();
        assert!(d.active_term().is_none());
        let a = d.add("p-blog");
        let b = d.add("p-blog"); // the same project can hold several
        assert_ne!(a, b);
        assert_eq!(d.active, 1, "a new tab is shown at once");
        d.active = 0;
        // Closing the shown one: it returns, and the slot falls to a neighbour.
        let t = d.remove(0).unwrap();
        assert_eq!(t.key, a);
        assert_eq!(d.active, 0);
        assert_eq!(d.active_term().unwrap().key, b);
        assert_eq!(d.len(), 1);
        // Closing the last leaves nothing to show.
        d.remove(0);
        assert!(d.active_term().is_none());
        assert!(d.remove(5).is_none());
    }
}
