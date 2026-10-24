//! The new project's form (the sidebar's "+"): a name and, if it has one, a folder on disk.

use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
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
                    )
                    .child(div().py_3().text_xs().text_color(muted).child(
                        "Its threads work in its folder. Servers, file hosts, the browser, tasks and \
                         tools are attached inside a thread with /.",
                    ))
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
    /// A project's environment at the top of its thread, as a document: the soul (edit in
    /// place), its MCP servers (JSON to edit), its skills (open one to read and edit), its
    /// accesses. No frames: headings, rows, text. A Save appears once a part changed; "+ Add"
    /// starts the request for the agent in the composer.
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
        let save = |id: &'static str, what: &'static str, cx: &mut Context<Self>| {
            h_flex()
                .gap_3()
                .child(act(format!("{id}-revert").into(), "Revert".into(), false).on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_revert(what, window, cx)
                })))
                .child(
                    div()
                        .id(SharedString::from(format!("{id}-save")))
                        .px_2()
                        .cursor_pointer()
                        .bg(theme.primary)
                        .text_color(theme.primary_foreground)
                        .hover(|s| s.bg(theme.primary_hover))
                        .child("Save")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.click_claimed = true;
                            this.env_save(what, window, cx)
                        })),
                )
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

        // The soul.
        let mut out = v_flex().w_full().gap_1();
        out = out
            .child(h_flex().gap_3().child(heading("Soul")).child(div().flex_1()).when(e.soul_dirty, |d| d.child(save("soul", "soul", cx))))
            // The field's own inset (12 / 10, as the composer's) pulled back: its text starts
            // where the headings do.
            .child(div().ml(px(-12.)).my(px(-10.)).child(gpui_kit::component::input::Textarea::new(&e.soul).appearance(false).large()));

        // MCP servers.
        let mcp_open = e.mcp_open;
        out = out.child(
            h_flex()
                .pt_4()
                .gap_3()
                .child(heading("MCP servers"))
                .child(add("env-mcp-add", "Add an MCP server to this project: ", cx))
                .child(act("env-mcp-json".into(), if mcp_open { "Hide JSON".into() } else { "JSON".into() }, false).on_click(cx.listener(|this, _, _, cx| {
                    this.click_claimed = true;
                    if let Some(e) = this.project_env.as_mut() {
                        e.mcp_open = !e.mcp_open;
                    }
                    cx.notify();
                })))
                .child(div().flex_1())
                .when(e.mcp_dirty && mcp_open, |d| d.child(save("mcp", "mcp", cx))),
        );
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
        if mcp_open {
            let lines = e.mcp_json.read(cx).value().lines().count().max(3) as f32;
            out = out.child(
                div()
                    .mt_1()
                    .h(px((lines + 1.) * 20.))
                    .child(gpui_kit::component::input::Editor::new(&e.mcp_json).appearance(false).h_full()),
            );
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
                    .when(open && e.skill_dirty, |d| d.child(save("skill", "skill", cx)))
                    .child(remove(id, "skill", name, cx)),
            );
            if open {
                out = out.child(
                    div()
                        .pl_4()
                        .pb_2()
                        .border_l_2()
                        .border_color(theme.border)
                        .child(div().ml(px(-12.)).my(px(-10.)).child(gpui_kit::component::input::Textarea::new(&e.skill_text).appearance(false).large())),
                );
            }
        }

        // Accesses.
        out = out.child(
            h_flex()
                .pt_4()
                .gap_3()
                .child(heading("Accesses"))
                .child(add("env-host-add", "Add to this project the access: ", cx)),
        );
        if e.hosts.is_empty() {
            out = out.child(div().text_color(muted).child("No servers or file hosts yet: /ssh and /ftp in its threads offer only these."));
        }
        for (hid, name, kind, at) in e.hosts.clone() {
            let id = format!("env-host-{hid}");
            out = out.child(row(id.clone(), name, format!("{kind} · {at}"), false).child(remove(id, "host", hid, cx)));
        }

        // Where it lives; the project's removal.
        let project_armed = is_armed("project:".into());
        out = out
            .when_some(e.error.clone(), |d, err| d.child(div().pt_2().text_color(theme.danger).child(err)))
            .child(
                h_flex()
                    .pt_4()
                    .gap_3()
                    .child(div().flex_1().min_w_0().text_color(muted).truncate().child(e.dir.clone()))
                    .child(
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
