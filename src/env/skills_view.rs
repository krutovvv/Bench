//! A skill's card drawn: its folder as a tree, the field naming a new file or folder, the open
//! file's card.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::section::code;
use super::skills::SkillInfo;
use crate::app::BenchApp;

impl BenchApp {
    /// The open skill's tree (folders open in place, a file opens under it; a row's rename and
    /// trash on hover), the field naming a new file or folder, and the open file's card.
    pub(super) fn skill_parts(
        &self,
        k: &SkillInfo,
        cx: &mut Context<Self>,
    ) -> (AnyElement, Option<AnyElement>, Option<AnyElement>) {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return (div().into_any_element(), None, None);
        };
        let shown = |rel: &str| {
            let mut up = rel;
            while let Some((p, _)) = up.rsplit_once('/') {
                if !e.skills.expanded.contains(p) {
                    return false;
                }
                up = p;
            }
            true
        };
        let mut rows: Vec<AnyElement> = vec![
            div()
                .px_3()
                .py_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(muted)
                .child(k.dir.clone())
                .into_any_element(),
        ];
        for (n, (rel, dir)) in e.skills.tree.iter().filter(|(r, _)| shown(r)).enumerate() {
            let (rel, dir) = (rel.clone(), *dir);
            let depth = rel.matches('/').count();
            let name = rel.rsplit('/').next().unwrap_or_default().to_string();
            let open = e.skills.expanded.contains(&rel);
            let chosen = !dir && e.skills.file.as_deref() == Some(rel.as_str());
            let group = SharedString::from(format!("env-skill-g{n}"));
            let armed = e.armed.as_deref() == Some(format!("skillfile:{rel}").as_str());
            let (ico, color) = crate::ui::file_icon(&name, dir, &theme);
            let chevron = if open {
                crate::icons::Ico::ChevronDown
            } else {
                crate::icons::Ico::ChevronRight
            };
            let (r1, r2) = (rel.clone(), rel.clone());
            let rename = crate::inserts::icon_button(
                &format!("env-skill-mv{n}"),
                crate::icons::Ico::Pencil,
                "Rename",
                &theme,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.click_claimed = true;
                cx.stop_propagation();
                this.skill_form("rename", r1.clone(), window, cx)
            }));
            let trash = div()
                .id(SharedString::from(format!("env-skill-rm{n}")))
                .flex_none()
                .px_1()
                .cursor_pointer()
                .text_sm()
                .when(!armed, |d| {
                    d.text_color(muted)
                        .hover(|s| s.text_color(theme.danger))
                        .child(crate::icons::Ico::Trash.icon().small())
                })
                .when(armed, |d| d.text_color(theme.danger).child("Delete?"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    this.skill_delete(r2.clone(), window, cx)
                }));
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("env-skill-row{n}")))
                    .group(group.clone())
                    .pl(px(12. + 16. * depth as f32))
                    .pr_3()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .border_t_1()
                    .border_color(theme.border.opacity(0.5))
                    .when(chosen, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(
                        div()
                            .w(px(12.))
                            .flex_none()
                            .text_color(muted)
                            .when(dir, |d| d.child(chevron.icon().xsmall())),
                    )
                    .child(ico.icon().small().text_color(color))
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .child(
                        h_flex()
                            .flex_none()
                            .when(!armed, |d| {
                                d.opacity(0.).group_hover(group, |s| s.opacity(1.))
                            })
                            .child(rename)
                            .child(trash),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.skill_entry(rel.clone(), dir, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        if e.skills.tree.is_empty() {
            rows.push(
                div()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(theme.border.opacity(0.5))
                    .text_color(muted)
                    .child("Reading the folder…")
                    .into_any_element(),
            );
        }
        let tree = v_flex().w_full().children(rows).into_any_element();

        let form = e.skills.form.as_ref().map(|(what, target)| {
            let title = match *what {
                "file" => "New file",
                "folder" => "New folder",
                _ => "Rename",
            };
            let place = if *what == "rename" {
                target.clone()
            } else if target.is_empty() {
                format!("In {}", k.name)
            } else {
                format!("In {}/{target}", k.name)
            };
            let actions = vec![
                crate::ui::form_action(
                    "env-skill-form-ok",
                    if *what == "rename" {
                        "Rename"
                    } else {
                        "Create"
                    },
                    true,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.click_claimed = true;
                    this.skill_form_done(window, cx)
                }))
                .into_any_element(),
                crate::ui::form_action("env-skill-form-cancel", "Cancel", false, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.click_claimed = true;
                        if let Some(e) = this.project_env.as_mut() {
                            e.skills.form = None;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            ];
            crate::ui::inline_form(
                title,
                vec![(
                    "Name",
                    gpui_kit::component::input::Input::new(&e.skills.form_name).into_any_element(),
                )],
                Some(place.into()),
                actions,
                cx,
            )
        });

        let file = e.skills.file.clone().map(|rel| {
            let dirty = e.skills.dirty;
            let save = div()
                .id("env-skill-file-save")
                .flex_none()
                .px_2()
                .rounded_md()
                .when(dirty, |d| {
                    d.cursor_pointer()
                        .bg(theme.primary)
                        .text_color(theme.primary_foreground)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.click_claimed = true;
                            this.env_save("skill", window, cx)
                        }))
                })
                .when(!dirty, |d| d.text_color(muted))
                .child("Save");
            let revert = dirty.then(|| {
                div()
                    .id("env-skill-file-revert")
                    .flex_none()
                    .cursor_pointer()
                    .text_color(muted)
                    .hover(|s| s.opacity(0.8))
                    .child("Revert")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_revert("skill", window, cx)
                    }))
            });
            let close = crate::inserts::icon_button(
                "env-skill-file-close",
                crate::icons::Ico::X,
                "Close the file",
                &theme,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.click_claimed = true;
                if let Some(e) = this.project_env.as_mut().filter(|e| !e.skills.dirty) {
                    e.skills.file = None;
                }
                cx.notify();
            }));
            let head = crate::ui::block_head(&theme, true)
                .gap_3()
                .child(crate::icons::Ico::File.icon().small())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.foreground)
                        .child(rel),
                )
                .child(save)
                .children(revert)
                .child(close);
            crate::inserts::card(head, code(&e.skills.text, cx), &theme)
        });
        (tree, form, file)
    }
}
