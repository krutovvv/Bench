//! SSH mode views: the mode switch beside the window buttons, the server list (folders from
//! `~/.ssh/config`), the header with the server's Hermes state, the server form.

use std::sync::atomic::Ordering;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;
use crate::icons::Ico;

/// The folder shown for servers without a `# folder:` of their own.
const DEFAULT_FOLDER: &str = "Servers";

impl HermesApp {
    /// The gateway menu in the status bar: This Mac + the servers of `~/.ssh/config` that run
    /// Hermes. Opening it re-reads the file and asks the servers again (for the next look).
    pub(super) fn render_target_picker(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let view = cx.entity();
        h_flex().h_full().items_center().pl_1().child(
            Button::new("target")
                .ghost()
                .xsmall()
                .icon(Ico::Server.icon())
                .label(self.target.label())
                .text_color(muted)
                .dropdown_menu(move |menu, _, cx| {
                    view.update(cx, |this, cx| this.refresh_targets(cx));
                    let targets = view.read(cx).targets.clone();
                    let current = view.read(cx).target.clone();
                    let mut menu = menu;
                    for t in targets {
                        let (v, t2) = (view.clone(), t.clone());
                        menu = menu.item(
                            PopupMenuItem::new(t.label())
                                .checked(t == current)
                                .on_click(move |_, window, cx| {
                                    v.update(cx, |this, cx| {
                                        this.pick_target(t2.clone(), window, cx)
                                    })
                                }),
                        );
                    }
                    menu
                }),
        )
    }

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
        let files = !self.ide_open()
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
            // The mode switch comes last: it changes the whole window.
            .child(
                toggle(
                    "mode-ssh",
                    Ico::Terminal,
                    ssh,
                    if ssh {
                        "Back to sessions  ⌘1"
                    } else {
                        "SSH servers  ⌘2"
                    },
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.set_ssh_mode(!ssh, window, cx)),
                ),
            )
    }

    pub(super) fn render_ssh_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let active = self.ssh.active_tab().map(|t| t.alias.clone());
        let editing = self.ssh.form.clone();
        let mut list: Vec<AnyElement> = vec![];
        for (g, (folder, rows)) in self.ssh.groups().into_iter().enumerate() {
            // Every server sits in a folder, like files in a tree; those without one of their own
            // (`# folder:`) in "Servers".
            {
                let hgroup = SharedString::from(format!("srv-folder-{g}"));
                let f = folder.clone();
                let key = format!("srv:{folder}");
                let collapsed = self.ssh.collapsed.contains(&key);
                let top = rems(if g > 0 { 0.75 } else { 0.5 });
                list.push(
                    h_flex()
                        .id(("srv-folder", g))
                        .group(hgroup.clone())
                        .cursor_pointer()
                        .px_4()
                        // A folder like in a file tree; its servers hang below on a guide.
                        .pt(top)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)),
                        )
                        .pb_1()
                        .gap_1p5()
                        .items_center()
                        .text_sm()
                        .child(Ico::Folder.icon().small().text_color(muted))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                // Muted like the icon, as the sessions' group names.
                                .text_color(muted)
                                .child(if folder.is_empty() {
                                    DEFAULT_FOLDER.to_string()
                                } else {
                                    folder.clone()
                                }),
                        )
                        .child(div().flex_1())
                        .child(super::group_chevron(collapsed, hgroup.clone(), cx))
                        .child(
                            div()
                                .id(("srv-folder-add", g))
                                .p_0p5()
                                .cursor_pointer()
                                .text_color(muted)
                                .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.foreground))
                                .child(Ico::Plus.icon().xsmall())
                                .tooltip(|window, cx| {
                                    Tooltip::new("New server in this folder").build(window, cx)
                                })
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.edit_server(None, Some(f.clone()), window, cx)
                                })),
                        )
                        .into_any_element(),
                );
            }
            // A folded folder shows its header only.
            if self.ssh.collapsed.contains(&format!("srv:{folder}")) {
                continue;
            }
            let n = rows.len();
            for (k, i) in rows.into_iter().enumerate() {
                let last = k + 1 == n;
                let s = &self.ssh.servers[i];
                let alias = s.alias.clone();
                let selected = match &editing {
                    Some(e) => e.as_deref() == Some(alias.as_str()),
                    None => active.as_deref() == Some(alias.as_str()),
                };
                let open = self.ssh.is_open(&alias);
                let armed = self.ssh.armed_delete.as_deref() == Some(alias.as_str());
                let group = SharedString::from(format!("srv-row-{i}"));
                let target = self
                    .ssh
                    .resolved
                    .get(&alias)
                    .cloned()
                    .unwrap_or_else(|| s.hostname.clone());
                let meta = format!("{target}{}", if open { " · connected" } else { "" });
                let compact = self.appearance.compact_sidebar;
                let (a1, a2, a3, a4) = (alias.clone(), alias.clone(), alias.clone(), alias.clone());
                list.push(
                    h_flex()
                        .id(("srv-row", i))
                        .group(group.clone())
                        .relative()
                        .gap_1()
                        .pl(super::nest_pl(cx))
                        .child(super::nest_guide(cx, last))
                        .pr_3()
                        .map(|d| if compact { d.py_1() } else { d.py_2() })
                        .border_l_2()
                        .border_color(if selected {
                            theme.primary
                        } else {
                            theme.transparent
                        })
                        .when(selected, |d| d.bg(theme.sidebar_accent))
                        .hover(|s| s.bg(theme.sidebar_accent))
                        .cursor_pointer()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .truncate()
                                        // Compact: no address line, so a connected server says
                                        // so in its name's color.
                                        .when(compact && open, |d| d.text_color(theme.primary))
                                        .child(alias.clone()),
                                )
                                .when(!compact, |d| {
                                    d.child(
                                        div()
                                            .text_xs()
                                            .truncate()
                                            .text_color(if open { theme.primary } else { muted })
                                            .child(meta),
                                    )
                                }),
                        )
                        // "⋯" on hover: edit, files, Zed, delete. An armed delete shows as a red
                        // trash button in its place until clicked again.
                        .map(|d| {
                            if armed {
                                return d.child(
                                    div()
                                        .id(("srv-delete", i))
                                        .p_1()
                                        .text_color(theme.danger)
                                        .child(Ico::Trash.icon().xsmall())
                                        .tooltip(|window, cx| {
                                            Tooltip::new("Click to remove from ~/.ssh/config")
                                                .build(window, cx)
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.delete_server(a2.clone(), window, cx)
                                        })),
                                );
                            }
                            let view = cx.entity();
                            let (a1, a4, a6) = (a1.clone(), a4.clone(), a2.clone());
                            let shared = s.shared;
                            d.child(
                                div()
                                    .opacity(0.)
                                    .group_hover(group, |s| s.opacity(1.))
                                    // Opening the menu must not also open the terminal.
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .child(
                                        Button::new(("srv-more", i))
                                            .ghost()
                                            .xsmall()
                                            .icon(Ico::Ellipsis.icon())
                                            .dropdown_menu(move |menu, _, _| {
                                                let act = |f: Box<
                                                    dyn Fn(
                                                        &mut HermesApp,
                                                        &mut Window,
                                                        &mut Context<HermesApp>,
                                                    ),
                                                >| {
                                                    let view = view.clone();
                                                    move |_: &ClickEvent,
                                                          window: &mut Window,
                                                          cx: &mut App| {
                                                        view.update(cx, |this, cx| {
                                                            f(this, window, cx)
                                                        })
                                                    }
                                                };
                                                let (e, f, x) =
                                                    (a1.clone(), a4.clone(), a6.clone());
                                                menu.item(
                                                    PopupMenuItem::new("Edit")
                                                        .icon(Ico::Pencil.icon())
                                                        .disabled(shared)
                                                        .on_click(act(Box::new(
                                                            move |this, window, cx| {
                                                                this.edit_server(
                                                                    Some(e.clone()),
                                                                    None,
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        ))),
                                                )
                                                .separator()
                                                .item(
                                                    PopupMenuItem::new("Open files")
                                                        .icon(Ico::FolderTree.icon())
                                                        .on_click(act(Box::new(
                                                            move |this, window, cx| {
                                                                this.open_ide_server(
                                                                    f.clone(),
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        ))),
                                                )
                                                .separator()
                                                .item(
                                                    PopupMenuItem::new("Delete")
                                                        .icon(Ico::Trash.icon())
                                                        .disabled(shared)
                                                        .on_click(act(Box::new(
                                                            move |this, window, cx| {
                                                                this.delete_server(
                                                                    x.clone(),
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        ))),
                                                )
                                            }),
                                    ),
                            )
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_server(a3.clone(), window, cx)
                        }))
                        .into_any_element(),
                );
            }
        }
        if self.ssh.servers.is_empty() {
            list.push(
                div()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .text_color(muted)
                    .child("No hosts in ~/.ssh/config yet")
                    .into_any_element(),
            );
        }

        let new_server = h_flex()
            .id("new-server")
            .h(rems(super::BAR_CONTROL_REMS))
            .px_3()
            .gap_2()
            .items_center()
            .justify_center()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .cursor_pointer()
            .hover(|s| s.bg(theme.primary_hover))
            .child(Ico::Plus.icon().small())
            .child("New server")
            .on_click(cx.listener(|this, _, window, cx| this.edit_server(None, None, window, cx)));

        v_flex()
            .size_full()
            .bg(theme.sidebar)
            .child(div().h(px(38.)).flex_none())
            .child(
                h_flex()
                    .flex_none()
                    .px_4()
                    .pb_2()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.sidebar_border)
                    .child(
                        Input::new(&self.ssh.search)
                            .cleanable(true)
                            .prefix(Ico::Search.icon().small().text_color(muted)),
                    ),
            )
            .child(
                v_flex()
                    .id("servers")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py_1()
                    .children(list)
                    .child(div().h_2().flex_none()),
            )
            .child(
                // Same height as the composer, so the rules line up across the window.
                h_flex()
                    .flex_none()
                    .h(super::bar_h(cx))
                    .px_4()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(new_server.flex_1()),
            )
            .into_any_element()
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
        .when_some(exited, |d, note| {
            d.child(div().text_xs().text_color(muted).child(note))
                .child(
                    toggle("reconnect", Ico::RotateCcw, "Reconnect").on_click(
                        cx.listener(|this, _, window, cx| this.reconnect_server(window, cx)),
                    ),
                )
        })
        .child(
            toggle(
                "disconnect",
                Ico::X,
                if local { "Close" } else { "Disconnect" },
            )
            .on_click(cx.listener(|this, _, window, cx| this.close_server_tab(window, cx))),
        )
        .child(self.render_header_tail(cx))
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
                         see the same server; the previous file is kept as config.bak-hermes-gpui. \
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
