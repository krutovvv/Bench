//! Rendering. Each file adds `render_*` methods to `HermesApp`.

mod asks;
mod chat;
mod composer;
mod files;
mod install;
mod media;
mod panels;
mod providers;
mod servers;
mod sidebar;
mod statusbar;
mod tasks;
mod terminal;
mod tools;

pub fn terminal_palette(theme: &gpui_kit::component::Theme) -> crate::term::Palette {
    terminal::palette(theme)
}

use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{HermesApp, Panel};

/// A sidebar's search: a band from edge to edge (rules above and below, the content's
/// background between), no frame of its own; the magnifier right over the group icons.
pub fn search_band(input: &Entity<gpui_kit::component::input::InputState>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .flex_none()
        .w_full()
        // As tall as the title bar: 36 with its two rules.
        .h(bar_h(cx) + px(1.))
        // Measured: the input's own padding plus the icon's side bearing put it 2.5 right of
        // the group icons at 6.
        .pl(px(3.5))
        .pr_2()
        .items_center()
        .bg(theme.background)
        .border_t_1()
        .border_b_1()
        .border_color(theme.sidebar_border)
        .child(
            gpui_kit::component::input::Input::new(input)
                .appearance(false)
                .cleanable(true)
                .prefix(
                    crate::icons::Ico::Search
                        .icon()
                        .small()
                        .text_color(theme.muted_foreground),
                ),
        )
}

/// Height of every bar (search band, "New session", the composer, a tag strip): the header's 36
/// less the 1px rule the bar draws (or shares) — 35 at the 14px font, following the
/// Appearance size. A bar with its rule is `bar_h + 1`.
pub fn bar_h(cx: &App) -> Pixels {
    cx.theme().font_size * 2.5
}

/// Rows under a group header (sessions of a project, servers of a folder) start where the
/// header's name does: 1rem inset + 1rem icon + 0.375rem gap, less the row's 2px stripe and
/// 2px the icon box is narrower than 1rem (measured).
impl HermesApp {
    /// The strip beside the window buttons at the top of every sidebar, with the mode toggles
    /// (Tools, then Tasks) in its top-right corner (the sidebar's own corner, as SSH has the
    /// window's).
    pub(super) fn render_sidebar_top(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        // A mode's toggle: its icon, lit while it is on; the tooltip says how to get back.
        let toggle = |id: &'static str, icon: crate::icons::Ico, name: &'static str, on: bool| {
            div()
                .id(id)
                .p_1()
                .cursor_pointer()
                .text_color(if on {
                    theme.primary
                } else {
                    theme.muted_foreground
                })
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                .child(icon.icon().small())
                .tooltip(move |window, cx| {
                    let tip = toggle_tip(name, on);
                    gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx)
                })
        };
        // 35 + the search band's top rule = the header's 36 with its bottom rule: the two rules
        // are one line across the window.
        h_flex()
            .h(px(35.))
            .flex_none()
            .justify_end()
            .items_center()
            .gap_1()
            // The icon's 4px box padding + 12 = the sidebar's 16px inset.
            .pr(px(12.))
            // The two utilities over the sidebar: Tools, then Tasks.
            .when(crate::bench::TOOLS, |d| {
                d.child(
                    toggle(
                        "mode-tools",
                        crate::icons::Ico::Wrench,
                        "Tools  ⌘3",
                        self.tools.on,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_tools(window, cx))),
                )
            })
            .when(crate::bench::TASKS, |d| {
                d.child(
                    toggle(
                        "mode-tasks",
                        crate::icons::Ico::ListTodo,
                        "Tasks  ⌘4",
                        self.tasks.on,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_tasks(window, cx))),
                )
            })
            .into_any_element()
    }
}

/// A mode toggle's tooltip: its name and shortcut, or "Back" and the shortcut while it is on.
/// Split at the double space, never by bytes: ⌘ is 3 bytes, and a byte slice through it panics,
/// which aborts the app (it did, on hovering the lit-up Tools toggle).
fn toggle_tip(name: &str, on: bool) -> String {
    match (on, name.rsplit_once("  ")) {
        (true, Some((_, keys))) => format!("Back  {keys}"),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn toggle_tip_keeps_the_shortcut_whole() {
        assert_eq!(super::toggle_tip("Tools  ⌘3", false), "Tools  ⌘3");
        assert_eq!(super::toggle_tip("Tools  ⌘3", true), "Back  ⌘3");
        assert_eq!(super::toggle_tip("Tasks  ⌘4", true), "Back  ⌘4");
        assert_eq!(super::toggle_tip("New window", true), "New window");
    }
}

pub fn nest_pl(cx: &App) -> Pixels {
    cx.theme().font_size * 2.375 - px(4.)
}

/// The group's indent guide: a 1px line under the center of the header's icon (1.5rem), less
/// the row's stripe. In a group's `last` row it stops where the text does, not in the row's
/// bottom padding (`py_2` = 0.5rem).
pub fn nest_guide(cx: &App, last: bool) -> impl IntoElement {
    div()
        .absolute()
        .top_0()
        .when(last, |d| d.bottom(cx.theme().font_size * 0.5))
        .when(!last, |d| d.bottom_0())
        .left(cx.theme().font_size * 1.5 - px(2.5))
        .w(px(1.))
        .bg(cx.theme().border)
}

/// A group header's fold chevron, at the right before its "+": shown on a folded group, and
/// on hover of an open one.
pub fn group_chevron(collapsed: bool, group: SharedString, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .p_0p5()
        .text_color(cx.theme().muted_foreground)
        .opacity(if collapsed { 1. } else { 0. })
        .group_hover(group, |s| s.opacity(1.))
        .child(
            if collapsed {
                crate::icons::Ico::ChevronRight
            } else {
                crate::icons::Ico::ChevronDown
            }
            .icon()
            .xsmall(),
        )
}

/// A 2-rem control's height (a filled button outside a bar, e.g. "Add server"). The bars
/// themselves are `bar_h`: a button that fills one takes `.h_full()` instead.
pub const BAR_CONTROL_REMS: f32 = 2.;

/// "1 action", "3 actions" (the `few` form is kept for call-site symmetry).
pub fn plural(n: usize, one: &str, _few: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn clock(secs: u64) -> String {
    if secs >= 60 {
        format!("{}:{:02}", secs / 60, secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub fn compact(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{}k", n / 1000),
        n => n.to_string(),
    }
}

pub fn clip(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

impl HermesApp {
    /// "app · hermes/fix-login": the chat's project and branch, with the git actions.
    fn render_project_chip(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use gpui_kit::component::button::{Button, ButtonVariants as _};
        use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
        let p = self.project.clone()?;
        let label = if p.branch.is_empty() {
            p.name()
        } else {
            format!("{} · {}", p.name(), p.branch)
        };
        let view = cx.entity();
        let armed = self.git_armed == Some(crate::app::GitArm::Delete);
        Some(
            Button::new("project-chip")
                .ghost()
                .xsmall()
                .icon(crate::icons::Ico::GitBranch.icon())
                .label(label)
                .dropdown_menu(move |menu, _, _| {
                    let act = |f: fn(&mut HermesApp, &mut Window, &mut Context<HermesApp>)| {
                        let view = view.clone();
                        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                            view.update(cx, |this, cx| f(this, window, cx))
                        }
                    };
                    let mut menu =
                        menu.item(PopupMenuItem::new("New chat in a new branch").on_click(act(
                            |this, _, cx| {
                                let root = this.project.as_ref().map(|p| p.root.clone());
                                this.new_chat_in(root, true, cx)
                            },
                        )));
                    if p.in_branch_folder() {
                        menu = menu
                            .item(
                                PopupMenuItem::new("Merge into main checkout").on_click(act(
                                    |this, window, cx| this.merge_branch(window, cx),
                                )),
                            )
                            .item(
                                PopupMenuItem::new(if armed {
                                    "Delete branch — confirm"
                                } else {
                                    "Delete branch"
                                })
                                .on_click(act(|this, window, cx| this.delete_branch(window, cx))),
                            );
                    }
                    menu
                })
                .into_any_element(),
        )
    }

    /// Header strip: the session title, or the settings tabs.
    fn render_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.tasks.on && self.panel == Panel::Chat {
            return self.render_tasks_header(cx);
        }
        if self.tools.on && self.panel == Panel::Chat {
            return self.render_tools_header(cx);
        }
        if self.ssh.on && self.panel == Panel::Chat {
            return self.render_ssh_header(cx);
        }
        let theme = cx.theme().clone();
        let bar = h_flex()
            .id("header")
            .h(px(36.))
            .flex_none()
            .bg(theme.title_bar)
            .items_center()
            .border_b_1()
            .border_color(theme.border);

        if !self.panel.is_settings() {
            let title: SharedString = self
                .session
                .as_ref()
                .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "New session".into())
                .into();
            return bar
                .pl_4()
                .pr_2()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(title),
                )
                .children(self.render_project_chip(cx))
                .child(self.render_header_tail(cx))
                .into_any_element();
        }

        let current = self.panel;
        let tabs = [
            ("tab-mcp", "MCP", Panel::Mcp),
            ("tab-skills", "Skills", Panel::Skills),
            ("tab-prompts", "Prompts", Panel::Prompts),
            ("tab-config", "Config", Panel::Config),
            ("tab-providers", "Providers", Panel::Providers),
            ("tab-messages", "Messages", Panel::Messages),
            ("tab-cron", "Cron", Panel::Cron),
            ("tab-gw", "Gateways", Panel::Gateways),
            ("tab-profiles", "Profiles", Panel::Profiles),
            ("tab-settings", "Settings", Panel::Settings),
        ]
        .map(|(id, label, panel)| {
            let active = panel == current;
            div()
                .id(id)
                .h_full()
                .px_4()
                .flex()
                .items_center()
                .text_sm()
                .cursor_pointer()
                .border_r_1()
                .border_color(theme.border)
                .when(active, |d| {
                    d.bg(theme.background).text_color(theme.foreground)
                })
                .when(!active, |d| {
                    d.text_color(theme.muted_foreground)
                        .hover(|s| s.text_color(theme.foreground))
                })
                .child(label)
                .on_click(
                    cx.listener(move |this, _, window, cx| this.open_panel(panel, window, cx)),
                )
        });
        bar.children(tabs)
            .child(div().flex_1())
            .child(
                div()
                    .id("settings-close")
                    .px_3()
                    .h_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_color(theme.muted_foreground)
                    .hover(|s| s.text_color(theme.foreground))
                    .child(crate::icons::Ico::X.icon().small())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.panel = Panel::Chat;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

impl Render for HermesApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The open chat's unsent text in the composer (app.rs, drafts).
        self.sync_draft(window, cx);
        let theme = cx.theme().clone();
        let ssh = self.ssh.on && self.panel == Panel::Chat;
        let body = match self.panel {
            Panel::Chat if self.tasks.on => self.render_tasks_body(cx),
            Panel::Chat if self.tools.on => self.render_tools_body(cx),
            Panel::Chat if ssh => self.render_ssh_body(window, cx),
            // A setup in a terminal (installing Hermes, a Claude login) has the chat area.
            Panel::Chat if self.setup.is_some() && self.setup_tab().is_some() => self
                .render_setup(window, cx)
                .unwrap_or_else(|| div().into_any_element()),
            Panel::Chat if self.hermes_missing => v_flex()
                .size_full()
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .child(self.render_hermes_missing(cx)),
                )
                .child(self.render_composer(window, cx))
                .into_any_element(),
            // Files dropped anywhere on the chat or the input are attached.
            Panel::Chat => v_flex()
                .id("chat-drop")
                .size_full()
                .drag_over::<ExternalPaths>(|s, _, _, cx| s.bg(cx.theme().list_active))
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    this.add_attachments(paths.paths().to_vec(), cx)
                }))
                .child(div().flex_1().min_h_0().child(self.render_chat(cx)))
                .children(self.render_plan(cx))
                .child(self.render_composer(window, cx))
                .into_any_element(),
            Panel::Skills => self.render_skills(cx).into_any_element(),
            Panel::Mcp => self.render_mcp(cx).into_any_element(),
            Panel::Config => self.render_config(cx).into_any_element(),
            Panel::Providers => self.render_providers(cx),
            Panel::Messages => self.render_messages(cx),
            Panel::Cron => self.render_cron(cx).into_any_element(),
            Panel::Settings => self.render_settings(cx).into_any_element(),
            Panel::Profiles => self.render_profiles(cx).into_any_element(),
            Panel::Gateways => self.render_gateways(cx).into_any_element(),
            Panel::Prompts => self.render_prompts(cx).into_any_element(),
        };

        let main = v_flex()
            .size_full()
            .min_w_0()
            .child(self.render_header(cx))
            .child(div().flex_1().min_h_0().child(body));

        // The IDE maximized (⌘⇧B) has the whole window; the rest keeps running, hidden.
        let expanded = self.ide.expanded && self.right.is_some() && self.panel == Panel::Chat;
        let content = self.render_layout(main, expanded, window, cx);
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            // Shortcuts for the whole window (app/keys.rs) see every key first.
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if this.global_key(&ev.keystroke, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .child(div().flex_1().min_h_0().child(content))
            .child(self.render_statusbar(window, cx))
    }
}

impl HermesApp {
    /// Sidebar | main | IDE (when open), resizable. Maximized, the IDE takes both places: its
    /// tree where the sidebar is (same width), its editor where the chat/terminal is.
    fn render_layout(
        &mut self,
        main: Div,
        expanded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // One left column, one IDE width: when the set of panels changes (maximized or not,
        // IDE on the right or not) the group starts from a fresh state laid out at the widths
        // remembered here — gpui-kit would otherwise squeeze the old sizes to fit a new panel.
        let shape = (expanded, self.right.is_some() && self.panel == Panel::Chat);
        if self.layout_synced != Some(shape) {
            self.layout_synced = Some(shape);
            if expanded {
                self.layout_ide_state = cx.new(|_| Default::default());
            } else {
                self.layout_state = cx.new(|_| Default::default());
            }
        }
        let state = if expanded {
            self.layout_ide_state.clone()
        } else {
            self.layout_state.clone()
        };
        let _ = window;
        // Every drag of a divider is that column's width from now on.
        let view = cx.entity();
        let remember = move |state: &Entity<gpui_kit::component::resizable::ResizableState>,
                             _: &mut Window,
                             cx: &mut App| {
            let sizes = state.read(cx).sizes().clone();
            view.update(cx, |this, _| {
                if let Some(w) = sizes.first() {
                    this.sidebar_w = *w;
                }
                // Normal layout with the IDE on the right: sidebar | main | IDE.
                if let (Some(w), false) = (sizes.get(2), this.ide.expanded) {
                    this.ide_w = *w;
                }
            });
        };
        // Fixed: a window resize goes to the middle; the columns keep their widths.
        let left = resizable_panel()
            .size(self.sidebar_w)
            .size_range(px(200.)..px(480.))
            .fixed();
        if expanded {
            return h_resizable("layout-ide")
                .with_state(&state)
                .on_resize(remember)
                .child(left.child(self.render_ide_sidebar(cx)))
                .child(resizable_panel().child(self.render_ide_main(cx)))
                .into_any_element();
        }
        h_resizable("layout")
            .with_state(&state)
            .on_resize(remember)
            .child(left.child(self.render_sidebar(cx)))
            .child(resizable_panel().child(main))
            .when(
                self.right.is_some()
                    && self.panel == Panel::Chat
                    && !self.tools.on
                    && !self.tasks.on,
                |d| {
                    d.child(
                        resizable_panel()
                            .size(self.ide_w)
                            .size_range(px(360.)..px(1000.))
                            .fixed()
                            .child(self.render_right(cx)),
                    )
                },
            )
            .into_any_element()
    }
}
