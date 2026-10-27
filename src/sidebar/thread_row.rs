//! A thread's row in the sidebar: its title, its state (running, unread, waiting), its mark, and
//! what it does on a click, a drag or a key.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::SESSION_TITLE;
use super::menu::item_menu;
use crate::app::BenchApp;
use crate::sidebar::items::Item;
use crate::workspace::Center;
use bench_ui::icons::Ico;
use bench_ui::plural;

impl BenchApp {
    /// `guide`: under a group header, indented on the group's guide line (`Some(last row)`).
    pub(super) fn render_session_row(
        &self,
        i: usize,
        guide: Option<bool>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let s = &self.sessions[i];
        let selected = self.center == Center::Chat && self.session.as_deref() == Some(&s.id);
        let group: SharedString = format!("srow{i}").into();
        let count = if s.running {
            "working…".to_string()
        } else {
            plural(s.messages as usize, "message", "messages", "messages")
        };
        // A chat in a branch of its own says which.
        let meta = match self.project_of(&s.id).filter(|p| p.in_branch_folder()) {
            Some(p) if !p.branch.is_empty() => format!("{} · {count}", p.branch),
            _ => count,
        };
        let (open_id, del_id) = (s.id.clone(), s.id.clone());
        // Deleting asks once (⌫, or the menu): the row shows Delete · Cancel; ⌫ again deletes.
        let armed = self.confirm_delete.as_deref() == Some(s.id.as_str());
        let compact = self.appearance.compact_sidebar;

        let delete = if armed {
            h_flex()
                .gap_1()
                .text_xs()
                .child(
                    div()
                        .id(("del-yes", i))
                        .px_1p5()
                        .py_0p5()
                        .rounded_md()
                        .bg(theme.danger)
                        .text_color(theme.danger_foreground)
                        .cursor_pointer()
                        .child("Delete")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.delete_session(del_id.clone(), window, cx)
                        })),
                )
                .child(
                    div()
                        .id(("del-no", i))
                        .px_1()
                        .py_0p5()
                        .cursor_pointer()
                        .text_color(muted)
                        .hover(|s| s.text_color(theme.foreground))
                        .child("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.cancel_delete(cx)
                        })),
                )
                .into_any_element()
        } else {
            // "⋯" on hover: pin, color, its files (the mini IDE; Zed is one click from there),
            // delete.
            let view = cx.entity();
            let id = s.id.clone();
            let mark = self.mark_of(&s.id);
            div()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                // Opening the menu must not also open the chat.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Button::new(("more", i))
                        .ghost()
                        .xsmall()
                        .icon(Ico::Ellipsis.icon())
                        .dropdown_menu(item_menu(
                            view.clone(),
                            Item::Thread(id.clone()),
                            mark.pinned,
                            None,
                        )),
                )
                .into_any_element()
        };
        // A chosen color marks the row with a quiet stripe (the open chat keeps the accent).
        let working =
            s.running || (self.live.is_some() && self.session.as_deref() == Some(s.id.as_str()));
        let stripe = self
            .mark_of(&s.id)
            .color
            .and_then(|c| crate::sidebar::marks::MARK_COLORS.get(c))
            .map(|c| -> Hsla { rgb(*c).into() });

        let pinned = self.mark_of(&s.id).pinned;
        let menu_id = s.id.clone();
        h_flex()
            .id(("session", i))
            .group(group)
            .relative()
            .gap_1()
            .pl(if guide.is_some() {
                bench_ui::nest_pl(cx)
            } else {
                px(14.)
            })
            .when_some(guide, |d, last| d.child(bench_ui::nest_guide(cx, last)))
            .pr_3()
            .map(|d| if compact { d.py_1p5() } else { d.py(px(5.)) })
            .cursor_pointer()
            .border_l_2()
            // A chosen color wins over the accent; it also tints the row lightly.
            .border_color(stripe.unwrap_or(if selected {
                theme.primary
            } else {
                theme.transparent
            }))
            .when_some(stripe, |d, c| {
                d.bg(Hsla {
                    a: if selected { 0.22 } else { 0.12 },
                    ..c
                })
            })
            // The open chat in the accent's own tint, not one more grey.
            .when(selected && stripe.is_none(), |d| {
                d.bg(theme.primary.opacity(0.12))
                    .text_color(theme.foreground)
            })
            // Hover keeps a chosen color, half a step denser; plain rows get the usual grey.
            .hover(move |h| match stripe {
                Some(c) => h.bg(Hsla {
                    a: if selected { 0.30 } else { 0.20 },
                    ..c
                }),
                None => h.bg(theme.sidebar_accent),
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    // A working session's title carries a moving highlight, to find it at a glance.
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .when(pinned, |d| {
                                d.child(Ico::Pin.icon().xsmall().text_color(muted))
                            })
                            .child(match self.renaming_field(&Item::Thread(s.id.clone())) {
                                // Renamed in place: the field where the title was.
                                // Over the title, in its place and size: the title stays (unseen) and
                                // keeps the row's height, so nothing moves.
                                Some(field) => div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(SESSION_TITLE))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_color(gpui_kit::transparent_black())
                                            .child(s.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .flex()
                                            .items_center()
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .child(
                                                bench_ui::bare_field(&field)
                                                    // Input draws at 0.875 of its size: the title's.
                                                    .with_size(gpui_kit::component::Size::Size(px(
                                                        SESSION_TITLE / 0.875,
                                                    )))
                                                    .px_0(),
                                            ),
                                    )
                                    .into_any_element(),
                                None => div()
                                    .min_w_0()
                                    .text_size(px(SESSION_TITLE))
                                    .truncate()
                                    .map(|d| {
                                        if working {
                                            d.child(ShimmerText::new(s.title.clone()))
                                        } else {
                                            d.child(s.title.clone())
                                        }
                                    })
                                    .into_any_element(),
                            }),
                    )
                    // Compact: the title alone (a working session still shimmers).
                    .when(!compact, |d| {
                        d.child(div().text_xs().text_color(muted).child(meta))
                    }),
            )
            .child(delete)
            // Opened, and the sidebar takes the keys: ↑ ↓ to the next, ⌫ deletes it.
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_session(open_id.clone(), cx);
                window.focus(&this.sidebar_focus, cx);
            }))
            .context_menu(item_menu(cx.entity(), Item::Thread(menu_id), pinned, None))
    }
}
