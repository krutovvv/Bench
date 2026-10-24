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
    /// A project's environment at the top of its thread: the soul (as text; Edit opens it), its
    /// MCP servers (on/off, remove), its skills (remove). A section's "+" starts the request in the
    /// composer; the agent does the rest, and the block follows.
    pub(super) fn render_env_block(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return div().text_color(muted).child("Loading…").into_any_element();
        };
        // A quiet text action: muted, the foreground on hover.
        let act = |id: SharedString, label: &'static str, danger: bool| {
            div()
                .id(id)
                .flex_none()
                .px_1()
                .cursor_pointer()
                .text_color(if danger { theme.danger } else { muted })
                .hover(|s| s.text_color(if danger { theme.danger } else { theme.foreground }))
                .child(label)
        };
        let head = |title: &'static str, add: Option<(&'static str, &'static str)>, cx: &mut Context<Self>| {
            h_flex()
                .pt_4()
                .pb_1()
                .gap_2()
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .children(add.map(|(id, ask)| {
                    act(id.into(), "+ Add", false).on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.ask_in_composer(ask, window, cx)
                    }))
                }))
        };
        let row = |id: SharedString, name: String, note: String, bad: bool| {
            h_flex()
                .id(id.clone())
                .group(id)
                .gap_2()
                .py_0p5()
                .child(div().flex_none().child(name))
                .child(div().flex_1().min_w_0().truncate().text_color(if bad { theme.danger } else { muted }).child(note))
        };
        let soul_text = e.soul.clone();
        let armed = e.armed.clone();
        let project_armed = armed.as_deref() == Some("project");
        let soul_all = e.soul_all;
        let skill_open = e.skill_open.clone();

        let soul_head = h_flex()
            .gap_2()
            .child(div().font_weight(FontWeight::MEDIUM).child("Soul"))
            .child(div().flex_1().min_w_0().truncate().text_color(muted).child("what the project is, how to work in it: ask below to change it"));
        let soul_body = if soul_text.trim().is_empty() {
            div().mt_1().text_color(muted).child("Not described yet: ask below (“describe the project: …”).").into_any_element()
        } else {
            // No scroll of its own (the thread scrolls): the first lines, Show all for the rest.
            const SHOWN: usize = 12;
            let lines = soul_text.lines().count();
            let shown = if soul_all || lines <= SHOWN { soul_text.clone() } else { soul_text.lines().take(SHOWN).collect::<Vec<_>>().join("\n") };
            v_flex()
                .mt_1()
                .px_3()
                .py_2()
                .border_1()
                .border_color(theme.border)
                .child(super::media::markdown("env-soul", shown))
                .when(lines > SHOWN, |d| {
                    d.child(
                        act("soul-all".into(), if soul_all { "Show less" } else { "Show all" }, false)
                            .pt_1()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.click_claimed = true;
                                if let Some(e) = this.project_env.as_mut() {
                                    e.soul_all = !e.soul_all;
                                }
                                cx.notify();
                            })),
                    )
                })
                .into_any_element()
        };

        let mcp_rows: Vec<AnyElement> = e
            .mcp
            .iter()
            .map(|(name, kind, enabled, connected, tools, error)| {
                let note = match (enabled, error, connected) {
                    (false, _, _) => format!("{kind} · off"),
                    (_, Some(err), _) => format!("{kind} · {err}"),
                    (_, _, true) => format!("{kind} · {tools} tools"),
                    _ => format!("{kind} · connects with the project's first thread"),
                };
                let gid: SharedString = format!("env-mcp-{name}").into();
                let is_armed = armed.as_deref() == Some(&format!("mcp:{name}"));
                let (n1, n2) = (name.clone(), name.clone());
                row(gid.clone(), name.clone(), note, error.is_some() && *enabled)
                    .child(
                        h_flex()
                            .flex_none()
                            .when(!is_armed, |d| d.opacity(0.).group_hover(gid.clone(), |s| s.opacity(1.)))
                            .child(act(format!("{gid}-on").into(), if *enabled { "Turn off" } else { "Turn on" }, false).on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                this.env_mcp(n1.clone(), "toggle", window, cx)
                            })))
                            .child(act(format!("{gid}-rm").into(), if is_armed { "Remove?" } else { "Remove" }, is_armed).on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                this.env_mcp(n2.clone(), "remove", window, cx)
                            }))),
                    )
                    .into_any_element()
            })
            .collect();
        let skill_rows: Vec<AnyElement> = e
            .skills
            .iter()
            .map(|(name, desc)| {
                let gid: SharedString = format!("env-skill-{name}").into();
                let is_armed = armed.as_deref() == Some(&format!("skill:{name}"));
                let n = name.clone();
                let open_name = name.clone();
                let text = skill_open.as_ref().filter(|(o, _)| o == name).map(|(_, t)| t.clone());
                let row = row(gid.clone(), name.clone(), desc.clone(), false)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_read_skill(open_name.clone(), window, cx)
                    }))
                    .child(
                        div()
                            .flex_none()
                            .when(!is_armed, |d| d.opacity(0.).group_hover(gid.clone(), |s| s.opacity(1.)))
                            .child(act(format!("{gid}-rm").into(), if is_armed { "Remove?" } else { "Remove" }, is_armed).on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                cx.stop_propagation();
                                this.env_remove_skill(n.clone(), window, cx)
                            }))),
                    );
                // Opened: its SKILL.md under the row, as text.
                v_flex()
                    .child(row)
                    .when_some(text, |d, t| {
                        d.child(
                            div()
                                .mt_1()
                                .mb_2()
                                .px_3()
                                .py_2()
                                .border_1()
                                .border_color(theme.border)
                                .child(super::media::markdown(format!("env-skill-text-{name}"), t)),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let (mcp_empty, skills_empty) = (mcp_rows.is_empty(), skill_rows.is_empty());
        let error = e.error.clone();
        let dir = e.dir.clone();
        v_flex()
            .w_full()
            .child(soul_head)
            .child(soul_body)
            .child(head("MCP servers", Some(("env-mcp-add", "Add an MCP server to this project: ")), cx))
            .when(mcp_empty, |d| d.child(div().text_color(muted).child("None of its own: the general ones work here too.")))
            .children(mcp_rows)
            .child(head("Skills", Some(("env-skill-add", "Write a skill for this project: ")), cx))
            .when(skills_empty, |d| d.child(div().text_color(muted).child("None of its own: the general ones work here too.")))
            .children(skill_rows)
            .when_some(error, |d, err| d.child(div().pt_2().text_color(theme.danger).child(err)))
            .child(
                h_flex()
                    .pt_4()
                    .gap_2()
                    .child(div().flex_1().min_w_0().text_color(muted).truncate().child(dir))
                    .child(
                        act("env-remove-project".into(), if project_armed { "Remove the project? Its threads stay" } else { "Remove project" }, project_armed)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.click_claimed = true;
                                this.env_remove_project(window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
}
