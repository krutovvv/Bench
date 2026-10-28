//! The bar under everything, the window's own: the agent's state on the left (its log, Reconnect,
//! Restart); terminals sent to the background, the thread beside and the terminal panel on the
//! right. What belongs to a thread (its context, its speed) is in the title bar by its repo
//! (`thread/meters.rs`).

mod agent;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;
use bench_ui::icons::Ico;

impl BenchApp {
    pub(crate) fn render_statusbar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let view = cx.entity();

        let item = |id: &'static str| {
            h_flex()
                .id(id)
                .h_full()
                .px_2()
                .gap_1()
                .items_center()
                .text_color(muted)
        };
        let button = |id: &'static str, icon: Ico, tip: &'static str, active: bool| {
            item(id)
                .cursor_pointer()
                // Muted from `item`; the accent while its panel is open, so set after it.
                .when(active, |d| d.text_color(theme.primary))
                .text_xs()
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
        };

        // A label as wide as its text that opens a menu of choices.
        // `note`: a line above the choices saying what they apply to.
        // A Button's `.label()` takes its font from the control's size, so bar labels are passed
        // as children with the bar's extra-small font.
        let picker = |id: &'static str,
                      icon: Ico,
                      label: String,
                      note: Option<&'static str>,
                      // (index of the first item, the group's name): a label above each group.
                      sections: Vec<(usize, String)>,
                      items: Vec<(
            String,
            bool,
            Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>,
        )>| {
            let items = std::rc::Rc::new(items);
            let view = view.clone();
            // Icon-only (an empty label): the icon sits alone in a square button, and the wrapper's
            // px_1 makes the inset 4+4 px a side — the same as the neighbouring items' px_2.
            // With a label: the icon sits inside the button, so hover lights up both.
            let icon_only = label.is_empty();
            let button = Button::new(id).ghost().small().icon(icon.icon());
            let button = if icon_only {
                button.text_xs()
            } else {
                button.child(div().text_xs().child(label))
            };
            let wrap = if icon_only {
                h_flex().h_full().items_center().px_1()
            } else {
                h_flex().h_full().items_center().pl_1()
            };
            wrap.child(
                button
                    // One tone across the bar, like the model and the icon buttons.
                    .text_color(muted)
                    .dropdown_menu(move |menu, _, _| {
                        let mut menu = menu;
                        if let Some(note) = note {
                            menu = menu.label(note).separator();
                        }
                        for (k, (text, checked, _)) in items.iter().enumerate() {
                            if let Some((_, name)) = sections.iter().find(|(at, _)| *at == k) {
                                if k > 0 {
                                    menu = menu.separator();
                                }
                                menu = menu.label(name.clone());
                            }
                            let (items, view) = (items.clone(), view.clone());
                            menu = menu.item(
                                PopupMenuItem::new(text.clone()).checked(*checked).on_click(
                                    move |_, window, cx| {
                                        view.update(cx, |this, cx| (items[k].2)(this, window, cx))
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            )
        };

        // Terminals sent to the background: each brings it back into its thread; Stop all ends them.
        let mut background_terms: Vec<(
            String,
            bool,
            Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)>,
        )> = self
            .inserts.ssh
            .tabs
            .iter()
            .filter(|t| t.background)
            .enumerate()
            .map(|(k, t)| {
                let id = t.id;
                let f: Box<dyn Fn(&mut BenchApp, &mut Window, &mut Context<BenchApp>)> =
                    Box::new(move |this, _, cx| {
                        if let Some(t) = this.inserts.ssh.tabs.iter_mut().find(|t| t.id == id) {
                            t.background = false;
                            t.collapsed = false;
                        }
                        cx.notify();
                    });
                (format!("Terminal {}", k + 1), false, f)
            })
            .collect();
        if !background_terms.is_empty() {
            background_terms.push((
                "Stop all".into(),
                false,
                Box::new(|this, _, cx| {
                    for t in this.inserts.ssh.tabs.iter().filter(|t| t.background) {
                        t.session.kill();
                    }
                    this.inserts.ssh.tabs.retain(|t| !t.background);
                    cx.notify();
                }),
            ));
        }
        h_flex()
            .h(px(bench_ui::STATUS_H - 1.))
            .flex_none()
            .w_full()
            .bg(theme.status_bar)
            .border_t_1()
            .border_color(theme.border)
            // The bar's font: a row's extra-small, like the header and the pane strips.
            .text_xs()
            // Far left: the agent of the machine this project lives on — always there, healthy
            // or not; its state in detail, its log, Reconnect and Restart are inside.
            .children(self.render_backend(cx))
            .child(div().flex_1())
            // Right: icons only — the terminals in the background (if any), attach, dictate,
            // settings, and Stop at the very edge.
            .when(!background_terms.is_empty(), |d| {
                d.child(picker(
                    "bg-terminals",
                    Ico::Terminal,
                    String::new(),
                    Some("Running terminals"),
                    vec![],
                    background_terms,
                ))
            })
            // The thread beside, whenever a document fills the center: the agent's sparkle,
            // before the terminal. It never hides while the thread is open — like the Explorer
            // and the terminal it is pressed while shown, and the second press closes it
            // (⌘J); the thread's own head keeps no cross of its own.
            .when(self.center == Center::Doc, |d| {
                let open = self
                    .shown_panel(crate::workspace::DockPosition::Right)
                    .is_some();
                d.child(
                    button(
                        "thread-beside",
                        Ico::Sparkles,
                        if open {
                            "Close the thread beside  ⌘J"
                        } else {
                            "The thread beside  ⌘J"
                        },
                        open,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.toggle_thread_beside(window, cx)),
                    ),
                )
            })
            // The bottom dock's terminal, on the project's machine (⌃`).
            .child({
                let open = self.bottom_dock.shows(crate::workspace::Panel::Terminal);
                button("terminal-panel", Ico::PanelBottom, "Terminal  ⌃`", open).on_click(
                    cx.listener(|this, _, window, cx| this.toggle_terminal_panel(window, cx)),
                )
            })
        // Attach and dictate left the status bar for the composer itself — they belong to
        // the message being written, not to the window's furniture.
        // No Stop here anymore: the hold and the stop ride the actions' own line in the
        // thread — where the minutes go, there the answers belong too (⌘. still works).
    }
}
