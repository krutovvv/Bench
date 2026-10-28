//! The title bar's right end, the window's top-right corner, as Claude's and Xcode's toolbars
//! keep the window's toggles: the terminals sent to the background (if any), the thread beside a
//! document (⌘J), the terminal panel below the center (⌃`), and Chat | Code (⌘E) at the edge.
//! Bench has no status bar: what the window toggles lives here, its agent in the sidebar's foot.

use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::{Center, DockPosition, Panel};
use bench_ui::icons::Ico;

impl BenchApp {
    pub(super) fn render_header_tail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ide = self.left_dock.shows(Panel::Explorer);
        let beside_open = self.shown_panel(DockPosition::Right).is_some();
        let terminal_open = self.bottom_dock.shows(Panel::Terminal);
        h_flex()
            .flex_none()
            .gap_1()
            .items_center()
            .children(self.render_background_terminals(cx))
            // The thread beside, whenever a document fills the center: pressed while shown, the
            // second press closes it; the thread's own head keeps no cross of its own.
            .when(self.center == Center::Doc, |d| {
                d.child(
                    bench_ui::icon_button(
                        "thread-beside",
                        Ico::Sparkles,
                        if beside_open {
                            "Close the thread beside  ⌘J"
                        } else {
                            "The thread beside  ⌘J"
                        },
                        beside_open,
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.toggle_thread_beside(window, cx)),
                    ),
                )
            })
            // The bottom dock's terminal, on the project's machine.
            .child(
                bench_ui::icon_button(
                    "terminal-panel",
                    Ico::PanelBottom,
                    "Terminal  ⌃`",
                    terminal_open,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.toggle_terminal_panel(window, cx))),
            )
            // The extreme corner, there whatever the center shows: the thread, or its project as
            // an IDE shows it — one hop either way, and which one is on is plain to see.
            .child({
                let app = cx.entity().downgrade();
                bench_ui::mode_switch(
                    "mode",
                    &[
                        (Ico::MessageSquare, "Chat", "The threads  ⌘E"),
                        (Ico::Code, "Code", "The project's files  ⌘E"),
                    ],
                    usize::from(ide),
                    cx,
                    std::rc::Rc::new(move |k, window, cx| {
                        if (k == 1) != ide {
                            let _ = app.update(cx, |this, cx| this.toggle_side(window, cx));
                        }
                    }),
                )
            })
            .into_any_element()
    }

    /// Terminals sent to the background, while there are any: a menu where each brings it back
    /// into its thread, and Stop all ends them.
    fn render_background_terminals(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ids: Vec<_> = self
            .inserts
            .ssh
            .tabs
            .iter()
            .filter(|t| t.background)
            .map(|t| t.id)
            .collect();
        if ids.is_empty() {
            return None;
        }
        let view = cx.entity();
        Some(
            Button::new("bg-terminals")
                .ghost()
                .small()
                .icon(Ico::Terminal.icon())
                .text_color(cx.theme().primary)
                .tooltip("Running terminals")
                .dropdown_menu(move |menu, _, _| {
                    let mut menu = menu.label("Running terminals").separator();
                    for (k, id) in ids.iter().copied().enumerate() {
                        let view = view.clone();
                        menu = menu.item(PopupMenuItem::new(format!("Terminal {}", k + 1)).on_click(
                            move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    if let Some(t) =
                                        this.inserts.ssh.tabs.iter_mut().find(|t| t.id == id)
                                    {
                                        t.background = false;
                                        t.collapsed = false;
                                    }
                                    cx.notify();
                                })
                            },
                        ));
                    }
                    let view = view.clone();
                    menu.separator().item(PopupMenuItem::new("Stop all").on_click(move |_, _, cx| {
                        view.update(cx, |this, cx| {
                            for t in this.inserts.ssh.tabs.iter().filter(|t| t.background) {
                                t.session.kill();
                            }
                            this.inserts.ssh.tabs.retain(|t| !t.background);
                            cx.notify();
                        })
                    }))
                })
                .into_any_element(),
        )
    }
}
