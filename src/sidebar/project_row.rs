//! A project in the sidebar: its head (an icon for what it is, the name, the fold, "+" for a new
//! thread in it), then its threads on the head's guide.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::sidebar::items::Item;
use super::menu::item_menu;
use crate::app::BenchApp;
use bench_ui::icons::Ico;

impl BenchApp {
    /// A project: its head (an icon for what it is, the name, the fold, "+" for a new thread in
    /// it), then its threads on the head's guide.
    pub(super) fn render_project(
        &mut self,
        g: usize,
        p: &crate::app::model::ProjectEntry,
        rows: Vec<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let key = format!("p:{}", p.id);
        let collapsed = self.ssh.collapsed.contains(&key);
        let group: SharedString = format!("phead{g}").into();
        let remote = !p.agent.is_empty();
        let icon = match (remote, &p.folder) {
            // A server's Host is the server; a project on it is a folder there, or none.
            (true, _) if p.is_host() => Ico::Server,
            (true, Some(_)) => Ico::Folder,
            (_, Some(f)) if std::path::Path::new(f).join(".git").exists() => Ico::FolderGit,
            (_, Some(_)) => Ico::Folder,
            _ => Ico::Layers,
        };
        // A remote project's agent: connecting, or why it is not there.
        let remote_note =
            Some(&p.agent)
                .filter(|_| remote)
                .and_then(|h| match self.agents.get(h) {
                    Some(r) if r.conn.is_some() => None,
                    Some(r) => Some(
                        r.error
                            .clone()
                            .map(|e| format!("{h}: {e}"))
                            .unwrap_or_else(|| format!("Connecting to {h}…")),
                    ),
                    None => Some(format!("{h}: not connected")),
                });
        let (new_id, menu_id) = (p.id.clone(), p.id.clone());
        let mark = self.mark_of(&format!("p:{}", p.id));
        let pinned = mark.pinned;
        // A chosen color is the project's icon and name, no background: it marks the group.
        let color: Option<Hsla> = mark
            .color
            .and_then(|c| crate::sidebar::marks::MARK_COLORS.get(c))
            .map(|c| rgb(*c).into());
        let armed = self.confirm_delete.as_deref() == Some(format!("p:{}", p.id).as_str());
        let renaming = self.renaming_field(&Item::Project(p.id.clone()));
        let zed = p
            .folder
            .clone()
            .filter(|f| !f.is_empty())
            // Zed opens it where its files are: this Mac, or that machine over SSH.
            .map(|f| {
                (
                    f,
                    Some(p.location().machine_here()).filter(|m| !m.is_empty()),
                )
            });
        let mut out = vec![
            h_flex()
                .id(("phead", g))
                .group(group.clone())
                .cursor_pointer()
                .px_4()
                .pt(rems(if g == 0 { 0.5 } else { 0.75 }))
                .pb_1()
                .gap_1p5()
                .items_center()
                .text_sm()
                .child(bench_ui::head_icon_in(icon, color, cx))
                .child(match renaming {
                    Some(field) => div()
                        .flex_1()
                        .min_w_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(Input::new(&field).xsmall())
                        .into_any_element(),
                    None => div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color.unwrap_or(muted))
                        .child(p.name.clone())
                        .into_any_element(),
                })
                .child(div().flex_1())
                // Removing asks first: its environment goes with it.
                .when(armed, |d| {
                    let yes = p.id.clone();
                    d.child(
                        h_flex()
                            .gap_1()
                            .text_xs()
                            .child(
                                div()
                                    .id(("premove-yes", g))
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(theme.danger)
                                    .text_color(theme.danger_foreground)
                                    .child("Remove")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.delete_project(yes.clone(), window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .id(("premove-no", g))
                                    .px_1()
                                    .py_0p5()
                                    .text_color(muted)
                                    .hover(|s| s.text_color(theme.foreground))
                                    .child("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.cancel_delete(cx)
                                    })),
                            ),
                    )
                })
                // "⋯" (on hover): the same menu as a right click.
                .child(
                    div()
                        .flex_none()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new(("pmore", g))
                                .ghost()
                                .xsmall()
                                .icon(Ico::Ellipsis.icon())
                                .dropdown_menu(item_menu(
                                    cx.entity(),
                                    Item::Project(menu_id.clone()),
                                    pinned,
                                    zed.clone(),
                                )),
                        ),
                )
                .child(
                    bench_ui::head_plus_slot()
                        .id(("pnew", g))
                        .cursor_pointer()
                        .text_color(muted)
                        // On hover, as the pencil.
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .hover(|s| s.bg(theme.sidebar_accent).text_color(theme.foreground))
                        .child(Ico::Plus.icon().xsmall())
                        .tooltip(|window, cx| {
                            Tooltip::new("New thread in this project").build(window, cx)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.new_thread_in(new_id.clone(), cx)
                        })),
                )
                // The fold, always the rightmost and always shown (open: down, folded: right).
                .child(
                    div().flex_none().p_0p5().text_color(muted).child(
                        if collapsed {
                            Ico::ChevronRight
                        } else {
                            Ico::ChevronDown
                        }
                        .icon()
                        .xsmall(),
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(key.clone(), cx)))
                .context_menu(item_menu(
                    cx.entity(),
                    Item::Project(menu_id.clone()),
                    pinned,
                    zed.clone(),
                ))
                .into_any_element(),
        ];
        if collapsed {
            return out;
        }
        if let Some(note) = remote_note {
            out.push(
                div()
                    .pl(bench_ui::nest_pl(cx))
                    .pr_4()
                    .py_1()
                    .text_sm()
                    .text_color(muted)
                    .truncate()
                    .child(note)
                    .into_any_element(),
            );
        }
        let n = rows.len();
        for (k, i) in rows.into_iter().enumerate() {
            out.push(
                self.render_session_row(i, Some(k + 1 == n), cx)
                    .into_any_element(),
            );
        }
        out
    }

}
