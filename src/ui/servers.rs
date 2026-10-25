//! SSH mode views: the mode switch beside the window buttons, the server list (folders from
//! `~/.ssh/config`), the header with the server's agent state, the server form.

use std::sync::atomic::Ordering;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::icons::Ico;

impl BenchApp {

    /// The header's right end in both modes: the SSH switch (pressed = SSH mode: servers and
    /// their terminals; released = sessions) and a new window. The IDE keeps its own close
    /// button at the far right of its panel.
    pub(super) fn render_header_tail(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ssh = self.ssh.on;
        let toggle = |id: &'static str, icon: Ico, on: bool, tip: &'static str| {
            div()
                .id(id)
                .p_1()
                .cursor_pointer()
                .text_color(if on {
                    theme.primary
                } else {
                    theme.muted_foreground
                })
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };
        // Files of what the window shows (the open chat, the server's terminal) one click
        // away, as ⌘B and "Open files" in the "⋯" menu; gone while the IDE is open (its own
        // close button stands there).
        let files = crate::bench::IDE
            && !self.ide_open()
            && !self.tools.on
            && !self.tasks.on
            && if ssh {
                self.ssh.active_tab().is_some()
            } else {
                self.session.is_some()
            };
        h_flex()
            .flex_none()
            .gap_1()
            .items_center()
            .when(files, |d| {
                d.child(
                    toggle("open-files", Ico::PanelRight, false, "Open files  ⌘B")
                        .on_click(cx.listener(|this, _, window, cx| this.toggle_ide(window, cx))),
                )
            })
            // A window of its own, in the window's top-right corner.
            .child(
                toggle("new-window", Ico::AppWindow, false, "New window  ⌘⇧N")
                    .on_click(cx.listener(|this, _, _, cx| this.open_new_window(cx))),
            )
    }

    pub(super) fn render_ssh_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let toggle = |id: &'static str, icon: Ico, tip: &'static str| {
            div()
                .id(id)
                .p_1()
                .cursor_pointer()
                .text_color(muted)
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };
        let bar = h_flex()
            .h(px(36.))
            .flex_none()
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.border)
            .pl_4()
            .pr_2()
            .gap_2()
            .items_center()
            .text_sm();
        if let Some(form) = &self.ssh.form {
            return bar
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .flex_1()
                        .child(if form.is_some() {
                            "Edit server"
                        } else {
                            "New server"
                        }),
                )
                .child(self.render_header_tail(cx))
                .into_any_element();
        }
        let Some(tab) = self.ssh.active_tab() else {
            return bar
                .child(div().flex_1())
                .child(self.render_header_tail(cx))
                .into_any_element();
        };
        let local = tab.alias == crate::term::LOCAL;
        let alias = if local {
            "This Mac".to_string()
        } else {
            tab.alias.clone()
        };
        let title = tab.session.title();
        let exited = tab.session.exited.load(Ordering::SeqCst).then(|| {
            tab.session
                .exit_note
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_default()
        });
        bar.child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .child(alias),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(muted)
                .children(title),
        )
        .when_some(exited.clone(), |d, note| {
            d.child(div().text_xs().text_color(muted).child(note))
        })
        // Every button on the right in one group, 4px apart like the header's own (the bar's
        // 8px gap had pushed the × away from its neighbours).
        .child(
            h_flex()
                .flex_none()
                .gap_1()
                .items_center()
                .when(exited.is_some(), |d| {
                    d.child(toggle("reconnect", Ico::RotateCcw, "Reconnect").on_click(
                        cx.listener(|this, _, window, cx| this.reconnect_server(window, cx)),
                    ))
                })
                .child(
                    toggle(
                        "disconnect",
                        Ico::X,
                        if local { "Close" } else { "Disconnect" },
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.close_server_tab(window, cx))),
                )
                .child(self.render_header_tail(cx)),
        )
        .into_any_element()
    }

    pub(super) fn render_ssh_body(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.ssh.form.is_some() {
            return self.render_server_form(cx);
        }
        match self.ssh.active {
            Some(ix) if ix < self.ssh.tabs.len() => self.render_terminal(ix, window, cx),
            _ => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(if self.ssh.servers.is_empty() {
                    "Add a server with “New server” below"
                } else {
                    "Pick a server on the left to connect"
                })
                .into_any_element(),
        }
    }

    /// A new (or edited) server, inside the thread: saved in ~/.ssh/config, then attached to it.
    pub(super) fn render_server_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.ssh.form.clone().flatten();
        let f = |e: &Entity<InputState>| Input::new(e).into_any_element();
        let rows = vec![
            ("Name", f(&self.ssh.f_alias)),
            ("Host", f(&self.ssh.f_host)),
            ("User", f(&self.ssh.f_user)),
            ("Port", f(&self.ssh.f_port)),
            ("Key file", f(&self.ssh.f_key)),
            ("Folder", f(&self.ssh.f_path)),
        ];
        let actions = vec![
            super::form_action("srv-save", if editing.is_some() { "Save and connect" } else { "Add and connect" }, true, cx)
                .on_click(cx.listener(|this, _, window, cx| this.save_server(window, cx)))
                .into_any_element(),
            super::form_action("srv-key", "Choose a key…", false, cx)
                .on_click(cx.listener(|this, _, window, cx| this.pick_key_file(window, cx)))
                .into_any_element(),
            super::form_action("srv-cancel", "Cancel", false, cx)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.ssh.form = None;
                    cx.notify();
                }))
                .into_any_element(),
        ];
        super::inline_form(
            if editing.is_some() { "Server" } else { "New server" },
            rows,
            Some("Saved in ~/.ssh/config (ssh signs in with your keys; no passwords are kept). Empty port = 22, empty folder = home.".into()),
            actions,
            cx,
        )
    }
}
