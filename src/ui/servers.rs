//! SSH mode views: the mode switch beside the window buttons, the server list (folders from
//! `~/.ssh/config`), the header with the server's agent state, the server form.

use std::sync::atomic::Ordering;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::icons::Ico;

impl HermesApp {

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

    fn render_server_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let editing = self.ssh.form.clone().flatten();
        let row = |title: &'static str, hint: &'static str, field: &Entity<InputState>| {
            h_flex()
                .gap_4()
                .py_3()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    v_flex()
                        .w(px(220.))
                        .flex_none()
                        .gap_0p5()
                        .child(div().text_sm().child(title))
                        .child(div().text_xs().text_color(muted).child(hint)),
                )
                .child(div().flex_1().child(Input::new(field)))
        };
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(rems(super::BAR_CONTROL_REMS))
                .px_4()
                .flex()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .when(primary, |d| {
                    d.bg(theme.primary)
                        .text_color(theme.primary_foreground)
                        .hover(|s| s.bg(theme.primary_hover))
                })
                .when(!primary, |d| {
                    d.border_1()
                        .border_color(theme.border)
                        .hover(|s| s.bg(theme.list_hover))
                })
                .child(label)
        };
        v_flex()
            .id("server-form")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .child(
                div()
                    .pt_4()
                    .pb_1()
                    .text_xs()
                    .text_color(muted)
                    .child("SERVER · ~/.ssh/config"),
            )
            .child(
                v_flex()
                    .max_w(px(720.))
                    .child(row(
                        "Name",
                        "The Host alias: ssh <name>.",
                        &self.ssh.f_alias,
                    ))
                    .child(row(
                        "Host",
                        "Address or domain (HostName).",
                        &self.ssh.f_host,
                    ))
                    .child(row("User", "Empty = your Mac user.", &self.ssh.f_user))
                    // The key: a path (IdentityFile), never a password.
                    .child(
                        h_flex()
                            .gap_4()
                            .py_3()
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                v_flex()
                                    .w(px(220.))
                                    .flex_none()
                                    .gap_0p5()
                                    .child(div().text_sm().child("Key file"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .child("Private key. Empty = ssh-agent."),
                                    ),
                            )
                            .child(div().flex_1().child(Input::new(&self.ssh.f_key)))
                            .child(button("srv-key", "Choose…", false).on_click(cx.listener(
                                |this, _, window, cx| this.pick_key_file(window, cx),
                            ))),
                    )
                    .child(row("Port", "Empty = 22.", &self.ssh.f_port))
                    .child(row(
                        "Folder",
                        "Groups servers in the list.",
                        &self.ssh.f_folder,
                    ))
                    .child(row(
                        "Default folder",
                        "Files and Zed open here. Empty = home.",
                        &self.ssh.f_path,
                    ))
                    .child(div().py_3().text_xs().text_color(muted).child(
                        "Saved in ~/.ssh/config: the connection and the key as ssh options \
                         (HostName, User, IdentityFile, Port), the folder and the default folder as \
                         # comments in the server's Host block (ssh ignores them), so there is one \
                         place for all of it. ssh, scp, Zed and other tools \
                         see the same server; the previous file is kept as config.bak-bench. \
                         No passwords are stored: ssh signs in with your keys and ssh-agent.",
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                button(
                                    "srv-save",
                                    if editing.is_some() {
                                        "Save and connect"
                                    } else {
                                        "Add and connect"
                                    },
                                    true,
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.save_server(window, cx)),
                                ),
                            )
                            .child(button("srv-cancel", "Cancel", false).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.ssh.form = None;
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .into_any_element()
    }
}
