//! A project in the sidebar: its head (an icon for what it is, the name, and on hover "⋯", the
//! gear (its environment in the Explorer) and "+" for a new thread in it; the fold is always
//! shown), then its threads on the head's guide.

use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::menu::item_menu;
use crate::app::BenchApp;
use crate::sidebar::items::Item;
use bench_ui::icons::Ico;

impl BenchApp {
    /// A project: its head and its threads on the head's guide (the controls of the head are in
    /// `menu.rs`'s notes: hover "⋯", gear, "+", and the fold always shown).
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
        let collapsed = self.sidebar.collapsed.contains(&key);
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
        let (new_id, menu_id, set_id) = (p.id.clone(), p.id.clone(), p.id.clone());
        let mark = self.mark_of(&format!("p:{}", p.id));
        let pinned = mark.pinned;
        // A chosen color is the project's icon and name, no background: it marks the group.
        let color: Option<Hsla> = mark
            .color
            .and_then(bench_ui::mark_color);
        let armed = self.armed.is(&format!("p:{}", p.id));
        let renaming = self.renaming_field(&Item::Project(p.id.clone()));
        let mut out = vec![
            h_flex()
                .id(("phead", g))
                .group(group.clone())
                .cursor_pointer()
                .px_4()
                .pt(if g == 0 {
                    AbsoluteLength::from(rems(0.5))
                } else {
                    AbsoluteLength::from(px(bench_ui::GROUP_GAP))
                })
                // One pixel more breath under a heading than a plain pb_1: projects are
                // sections, and the eye wants the groups further apart than the threads inside.
                .pb(px(bench_ui::GROUP_HEAD_PB))
                .gap_1p5()
                .items_center()
                // The list's size, a heavier weight: a project is the name its threads hang
                // under, read as a word and not as a label in capitals; a color marks it.
                .text_size(px(bench_ui::SIDEBAR_TEXT))
                .child(bench_ui::head_icon_in(icon, color, cx))
                .child(match renaming {
                    Some(field) => div()
                        .flex_1()
                        .min_w_0()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(Input::new(&field).xsmall())
                        .into_any_element(),
                    // The name in the text's colour; only a chosen color marks it.
                    None => div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(color.unwrap_or(theme.foreground))
                        .child(p.name.clone())
                        .into_any_element(),
                })
                .child(div().flex_1())
                // Removing asks first: its environment goes with it.
                .when(armed, |d| {
                    let yes = p.id.clone();
                    d.child(bench_ui::confirm_buttons(
                        format!("premove-{g}"),
                        "Remove",
                        Box::new(cx.listener(move |this, _, window, cx| {
                            this.delete_project(yes.clone(), window, cx)
                        })),
                        Box::new(cx.listener(|this, _, _, cx| this.cancel_delete(cx))),
                        cx,
                    ))
                })
                // "⋯" (on hover): the same menu as a right click.
                .child(
                    bench_ui::on_group_hover(group.clone())
                        .child(
                            bench_ui::row_icon_button(("pmore", g), Ico::Ellipsis)
                                .dropdown_menu(item_menu(
                                    cx.entity(),
                                    Item::Project(menu_id.clone()),
                                    pinned,
                                )),
                        ),
                )
                // The gear (on hover): the project's environment, in the Explorer. On the row, not
                // in the menu — a door to Settings that only shows on a right click is a door
                // nobody finds; the gear says settings, and what it opens says there is more.
                .child(
                    bench_ui::on_group_hover(group.clone())
                        .child(
                            bench_ui::row_icon_button(("pset", g), Ico::Settings)
                                .tooltip("Settings")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.open_project_settings(set_id.clone(), window, cx)
                                })),
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
                .context_menu(item_menu(cx.entity(), Item::Project(menu_id), pinned))
                .into_any_element(),
        ];
        if collapsed {
            return out;
        }
        if let Some(note) = remote_note {
            out.push(
                div()
                    .pl(px(bench_ui::PROJECT_NAME_X))
                    .pr_4()
                    .py_1()
                    // The note belongs to the head: the heading's own size.
                    .text_xs()
                    .text_color(muted)
                    .truncate()
                    .child(note)
                    .into_any_element(),
            );
        }
        let n = rows.len();
        for (k, i) in rows.into_iter().enumerate() {
            out.push(
                self.render_thread_row(i, Some(k + 1 == n), cx)
                    .into_any_element(),
            );
        }
        out
    }
}
