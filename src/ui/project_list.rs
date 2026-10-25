//! The new project's form (the sidebar's "+"): a name and, if it has one, a folder on disk.

use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::HermesApp;

impl HermesApp {
    pub(super) fn render_project_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(form) = self.project_form.as_ref() else {
            return div().into_any_element();
        };
        let label = |title: &'static str, hint: &'static str| {
            v_flex()
                .w(px(220.))
                .flex_none()
                .gap_0p5()
                .child(div().text_sm().child(title))
                .child(div().text_xs().text_color(muted).child(hint))
        };
        let button = |id: &'static str, text: &'static str, primary: bool| {
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
                .when(!primary, |d| d.border_1().border_color(theme.border).hover(|s| s.bg(theme.list_hover)))
                .child(text)
        };
        let folder = form.folder.clone();
        let host = form.host.clone();
        // Where its agent runs: this Mac, or one of the ~/.ssh/config servers.
        let mut places: Vec<(Option<String>, String)> = vec![(None, "This Mac".into())];
        places.extend(self.ssh.servers.iter().map(|s| (Some(s.alias.clone()), s.alias.clone())));
        let where_row = h_flex().flex_wrap().gap_1().children(places.into_iter().enumerate().map(|(k, (h, label))| {
            let on = h == host;
            div()
                .id(("project-where", k))
                .px_2()
                .py_0p5()
                .text_sm()
                .cursor_pointer()
                .border_1()
                .border_color(if on { theme.primary } else { theme.border })
                .when(on, |d| d.bg(theme.primary).text_color(theme.primary_foreground))
                .when(!on, |d| d.hover(|s| s.bg(theme.list_hover)))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(f) = this.project_form.as_mut() {
                        f.host = h.clone();
                    }
                    cx.notify();
                }))
        }));
        v_flex()
            .id("project-form")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .child(div().pt_4().pb_1().text_xs().text_color(muted).child("NEW PROJECT"))
            .child(
                v_flex()
                    .max_w(px(720.))
                    .child(
                        h_flex()
                            .gap_4()
                            .py_3()
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(label("Name", "What you work on: a site, a client, a kind of work."))
                            .child(div().flex_1().child(Input::new(&form.name))),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .py_3()
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(label("Where", "This Mac, or a server: its agent there keeps the threads, skills and MCP."))
                            .child(div().flex_1().child(where_row)),
                    )
                    .when(host.is_none(), |d| d.child(
                        h_flex()
                            .gap_4()
                            .py_3()
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(label("Folder", "Its code or files on disk, if it has any. Optional."))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .text_color(if folder.is_some() { theme.foreground } else { muted })
                                    .child(folder.clone().unwrap_or_else(|| "None: the project lives in Bench".into())),
                            )
                            .child(button("project-folder", "Choose…", false).on_click(
                                cx.listener(|this, _, window, cx| this.pick_project_codebase(window, cx)),
                            )),
                    ))
                    .child(div().py_3().text_xs().text_color(muted).child(if host.is_some() {
                        "Bench installs and starts the agent on that server over SSH (Node.js 22.18+ there) \
                         and works with it through a tunnel; on its first start it takes over what Hermes \
                         kept there: the default profile's skills, MCP servers, sessions."
                    } else {
                        "Its threads work in its folder. Servers, file hosts, the browser, tasks and \
                         tools are attached inside a thread with /."
                    }))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(button("project-save", "Create", true).on_click(cx.listener(|this, _, window, cx| this.save_project(window, cx))))
                            .child(button("project-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                                this.project_form = None;
                                cx.notify();
                            }))),
                    ),
            )
            .into_any_element()
    }
}

impl HermesApp {
    /// A project's environment at the top of its thread, as a document of rows: its soul, its
    /// Telegram bot, its MCP servers and their JSON, its skills, its accesses. A row opens in
    /// place (a rule on its left, the text to edit, Save / Revert under it) and closes again;
    /// "+ Add" starts the request for the agent in the composer.
    pub(super) fn render_env_block(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return div().text_color(muted).child("Loading…").into_any_element();
        };
        // A quiet text action: muted, the foreground on hover; red when it asks to confirm.
        let act = |id: SharedString, label: String, danger: bool| {
            div()
                .id(id)
                .flex_none()
                .cursor_pointer()
                .text_color(if danger { theme.danger } else { muted })
                .hover(|s| s.text_color(if danger { theme.danger } else { theme.foreground }))
                .child(label)
        };
        // Under an opened row: Save (live once something changed) and Revert.
        let footer = |id: &'static str, what: &'static str, dirty: bool, cx: &mut Context<Self>| {
            h_flex()
                .pt_1()
                .gap_3()
                .child(
                    div()
                        .id(SharedString::from(format!("{id}-save")))
                        .px_2()
                        .when(dirty, |d| {
                            d.cursor_pointer()
                                .bg(theme.primary)
                                .text_color(theme.primary_foreground)
                                .hover(|s| s.bg(theme.primary_hover))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.click_claimed = true;
                                    this.env_save(what, window, cx)
                                }))
                        })
                        .when(!dirty, |d| d.border_1().border_color(theme.border).text_color(muted))
                        .child("Save"),
                )
                .when(dirty, |d| {
                    d.child(act(format!("{id}-revert").into(), "Revert".into(), false).on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_revert(what, window, cx)
                    })))
                })
        };
        // An opened row's body: a rule on its left, the content, its footer.
        let opened = |content: AnyElement, foot: AnyElement| {
            v_flex().mt_0p5().mb_2().pl_4().border_l_2().border_color(theme.primary.opacity(0.5)).child(content).child(foot)
        };
        let heading = |title: &'static str| div().font_weight(FontWeight::MEDIUM).child(title);
        let armed = e.armed.clone();
        let is_armed = |k: String| armed.as_deref() == Some(&k);
        let remove = |id: String, what: &'static str, name: String, cx: &mut Context<Self>| {
            let on = is_armed(format!("{what}:{name}"));
            div()
                .flex_none()
                .when(!on, |d| d.opacity(0.).group_hover(SharedString::from(id.clone()), |s| s.opacity(1.)))
                .child(act(format!("{id}-rm").into(), if on { "Remove?".into() } else { "Remove".into() }, on).on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    this.env_remove(what, name.clone(), window, cx)
                })))
        };
        let add = |id: &'static str, ask: &'static str, cx: &mut Context<Self>| {
            act(id.into(), "+ Add".into(), false).on_click(cx.listener(move |this, _, window, cx| {
                this.click_claimed = true;
                this.ask_in_composer(ask, window, cx)
            }))
        };
        let row = |id: String, name: String, note: String, bad: bool| {
            h_flex()
                .id(SharedString::from(id.clone()))
                .group(SharedString::from(id))
                .gap_2()
                .py_0p5()
                .child(div().flex_none().child(name))
                .child(div().flex_1().min_w_0().truncate().text_color(if bad { theme.danger } else { muted }).child(note))
        };
        let toggle = |which: &'static str| {
            move |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                this.click_claimed = true;
                if let Some(e) = this.project_env.as_mut() {
                    match which {
                        "soul" => e.soul_open = !e.soul_open,
                        "telegram" => e.tg_open = !e.tg_open,
                        _ => e.mcp_open = !e.mcp_open,
                    }
                }
                cx.notify();
            }
        };
        // An opened text: the editor as tall as its wrapped text (the thread scrolls, not it).
        let text_editor = |state: &Entity<gpui_kit::component::input::EditorState>, cx: &App| {
            let text = state.read(cx).value().to_string();
            // Wrapped lines, roughly: a line per ~110 characters of the column.
            let lines: usize = text.lines().map(|l| l.chars().count() / 110 + 1).sum::<usize>().max(2);
            // The editor's own 12 inset pulled back: its text starts where the Save under it does.
            div().ml(px(-12.)).h(px(lines as f32 * 21. + 12.)).child(gpui_kit::component::input::Editor::new(state).appearance(false).h_full()).into_any_element()
        };
        let mut out = v_flex().w_full();

        // The soul: a row with its first line; opened, the text to edit.
        let soul_text = e.soul.read(cx).value().to_string();
        let first = soul_text.lines().map(|l| l.trim_start_matches('#').trim()).find(|l| !l.is_empty()).unwrap_or("not described yet").to_string();
        out = out.child(
            row("env-soul".into(), "Soul".into(), if e.soul_open { "what the project is, how to work in it".into() } else { first }, false)
                .cursor_pointer()
                .on_click(cx.listener(toggle("soul"))),
        );
        if e.soul_open {
            let body = text_editor(&e.soul, cx);
            out = out.child(opened(body, footer("soul", "soul", e.soul_dirty, cx).into_any_element()));
        }

        // Its Telegram bot.
        let tg_note = match &e.telegram {
            Some((name, users, true)) => format!("{} · {} may write", if name.is_empty() { "a bot".to_string() } else { format!("@{name}") }, users.len()),
            _ => "no bot of its own: open to set one".into(),
        };
        out = out.child(row("env-tg".into(), "Telegram".into(), tg_note, false).cursor_pointer().on_click(cx.listener(toggle("telegram"))));
        if e.tg_open {
            let body = v_flex()
                .gap_2()
                .child(gpui_kit::component::input::Input::new(&e.tg_token))
                .child(gpui_kit::component::input::Input::new(&e.tg_users))
                .child(div().text_color(muted).child("Its chats become this project's threads, with its soul, skills and servers."))
                .into_any_element();
            out = out.child(opened(body, footer("tg", "telegram", e.tg_dirty, cx).into_any_element()));
        }

        // MCP servers, and their JSON.
        out = out.child(h_flex().pt_4().gap_3().child(heading("MCP servers")).child(add("env-mcp-add", "Add an MCP server to this project: ", cx)));
        if e.mcp.is_empty() {
            out = out.child(div().text_color(muted).child("None of its own: the general ones work here too."));
        }
        for (name, kind, enabled, connected, tools, error) in e.mcp.clone() {
            let note = match (enabled, &error, connected) {
                (false, _, _) => format!("{kind} · off (\"enabled\": false)"),
                (_, Some(err), _) => format!("{kind} · {err}"),
                (_, _, true) => format!("{kind} · {tools} tools"),
                _ => format!("{kind} · connects with the project's first thread"),
            };
            let id = format!("env-mcp-{name}");
            out = out.child(row(id.clone(), name.clone(), note, error.is_some() && enabled).child(remove(id, "mcp", name, cx)));
        }
        out = out.child(
            row("env-mcp-json".into(), "mcp.json".into(), if e.mcp_open { "the servers as JSON".into() } else { "edit the servers as JSON".into() }, false)
                .cursor_pointer()
                .on_click(cx.listener(toggle("mcp"))),
        );
        if e.mcp_open {
            let body = text_editor(&e.mcp_json, cx);
            out = out.child(opened(body, footer("mcp", "mcp", e.mcp_dirty, cx).into_any_element()));
        }

        // Skills.
        out = out.child(h_flex().pt_4().gap_3().child(heading("Skills")).child(add("env-skill-add", "Write a skill for this project: ", cx)));
        if e.skills.is_empty() {
            out = out.child(div().text_color(muted).child("None of its own: the general ones work here too."));
        }
        for (name, desc) in e.skills.clone() {
            let id = format!("env-skill-{name}");
            let open = e.skill_open.as_deref() == Some(&name);
            let n = name.clone();
            out = out.child(
                row(id.clone(), name.clone(), desc, false)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_open_skill(n.clone(), window, cx)
                    }))
                    .child(remove(id, "skill", name, cx)),
            );
            if open {
                let body = text_editor(&e.skill_text, cx);
                out = out.child(opened(body, footer("skill", "skill", e.skill_dirty, cx).into_any_element()));
            }
        }

        // Accesses.
        out = out.child(h_flex().pt_4().gap_3().child(heading("Accesses")).child(add("env-host-add", "Add to this project the access: ", cx)));
        if e.hosts.is_empty() {
            out = out.child(div().text_color(muted).child("No servers or file hosts yet: /ssh and /ftp in its threads offer only these."));
        }
        for (hid, name, kind, at) in e.hosts.clone() {
            let id = format!("env-host-{hid}");
            out = out.child(row(id.clone(), name, format!("{kind} · {at}"), false).child(remove(id, "host", hid, cx)));
        }

        // The project's removal.
        let project_armed = is_armed("project:".into());
        out = out
            .when_some(e.error.clone(), |d, err| d.child(div().pt_2().text_color(theme.danger).child(err)))
            .child(
                h_flex().pt_4().child(div().flex_1()).child(
                    act("env-remove-project".into(), if project_armed { "Remove the project? Its threads stay".into() } else { "Remove project".into() }, project_armed)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.click_claimed = true;
                            this.env_remove("project", String::new(), window, cx)
                        })),
                ),
            );
        out.into_any_element()
    }
}
