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
    /// A project's environment at the top of its thread: the soul (editable), its MCP servers,
    /// its skills. The conversation under it changes the same things through the agent.
    pub(super) fn render_env_block(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return div().text_color(muted).child("Loading…").into_any_element();
        };
        let section = |title: &'static str| div().pt_3().pb_1().text_color(muted).child(title);
        let row = |name: String, note: String, bad: bool| {
            h_flex()
                .gap_2()
                .py_0p5()
                .child(div().flex_none().child(name))
                .child(div().min_w_0().truncate().text_color(if bad { theme.danger } else { muted }).child(note))
        };
        let dirty = e.dirty;
        v_flex()
            .w_full()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().text_color(muted).child("Soul: what the project is, how to work in it"))
                    .child(div().flex_1())
                    .when(dirty, |d| {
                        d.child(
                            div()
                                .id("soul-save")
                                .px_2()
                                .cursor_pointer()
                                .bg(theme.primary)
                                .text_color(theme.primary_foreground)
                                .hover(|s| s.bg(theme.primary_hover))
                                .child("Save")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.click_claimed = true;
                                    this.save_soul(window, cx)
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .mt_1()
                    .h(px(180.))
                    .border_1()
                    .border_color(theme.border)
                    .child(gpui_kit::component::input::Editor::new(&e.soul).appearance(false).h_full()),
            )
            .child(section("MCP servers"))
            .when(e.mcp.is_empty(), |d| d.child(div().text_color(muted).child("None of its own yet: ask below to add one.")))
            .children(e.mcp.iter().map(|(name, kind, enabled, tools, error)| {
                let note = match (enabled, error) {
                    (false, _) => format!("{kind} · off"),
                    (_, Some(err)) => format!("{kind} · {err}"),
                    _ => format!("{kind} · {tools} tools"),
                };
                row(name.clone(), note, error.is_some())
            }))
            .child(section("Skills"))
            .when(e.skills.is_empty(), |d| d.child(div().text_color(muted).child("None of its own yet: ask below to write one.")))
            .children(e.skills.iter().map(|(name, desc)| row(name.clone(), desc.clone(), false)))
            .when_some(e.error.clone(), |d, err| d.child(div().pt_2().text_color(theme.danger).child(err)))
            .child(div().pt_3().text_color(muted).truncate().child(e.dir.clone()))
            .into_any_element()
    }
}
