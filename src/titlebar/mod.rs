//! The bar over the center: the thread's title, its meters (`thread/meters.rs`) and its repo's
//! branch, the documents' project
//! (their tabs are the center's own bar, `explorer/view.rs`) with the thread-beside button, or a
//! settings page's title and its close.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::*;

use crate::app::BenchApp;
use crate::workspace::Center;

impl BenchApp {
    /// The header's right end, the window's top-right corner: the Explorer's button — it opens
    /// the project of the thread open now, and is pressed while that tree is shown (⌘E).
    /// The thread-beside toggle lives by the terminal, in the status bar. Shown,
    /// the thread's own head takes this corner with its + and ×.
    fn render_header_tail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let ide = self.left_dock.shows(crate::workspace::Panel::Explorer);
        h_flex()
            .flex_none()
            .gap_1()
            .items_center()
            // The extreme corner, there whatever the center shows: one hop from a thread to its
            // project's files, and back by the same pressed button.
            .child(
                bench_ui::icon_button(
                    "ide-toggle",
                    bench_ui::icons::Ico::FolderTree,
                    if ide {
                        "Close the Explorer  ⌘E"
                    } else {
                        "The project's files  ⌘E"
                    },
                    ide,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.toggle_side(window, cx))),
            )
            .into_any_element()
    }

    /// "app · main": the thread works in a git repository (its folder's name and branch). A
    /// click opens what the repo is: uncommitted changes, the newest commits (a click copies
    /// the sha), its origin. Git itself stays the agent's: ask it to commit, or to undo.
    fn render_project_chip(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.thread(cx).repo.clone()?;
        let label = if p.branch.is_empty() {
            p.name()
        } else {
            format!("{} · {}", p.name(), p.branch)
        };
        let at = self.thread_ref(cx);
        let dir = p.dir.clone();
        if !self.git.info.contains_key(&dir) {
            let target = self.thread_target(&at);
            self.refresh_git(target, dir.clone(), cx);
        }
        let theme = cx.theme();
        let info = self.git.info.get(&dir).cloned();
        let app = cx.entity().downgrade();
        let copy = |app: gpui::WeakEntity<BenchApp>, shown: String, what: String| {
            PopupMenuItem::new(shown).on_click(move |_, window, cx| {
                let what = what.clone();
                let _ = app.update(cx, |this, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(what.clone()));
                    this.toast(NotificationType::Info, format!("Copied {what}"), window, cx);
                });
            })
        };
        Some(
            Button::new("repo-chip")
                .ghost()
                .small()
                .text_color(theme.muted_foreground)
                .icon(bench_ui::icons::Ico::GitBranch.icon().small())
                // A Button's `.label()` takes the control's size: the bars' size is set here.
                .child(div().text_xs().child(label))
                .dropdown_menu(move |menu, _, _| {
                    // One line each, clipped: a long subject or path must not grow the menu
                    // wider than the window (clicking still copies the whole thing).
                    let mut menu = menu.item(PopupMenuItem::label(bench_ui::clip(&dir, 64)));
                    let Some(info) = info.clone() else {
                        return menu.item(PopupMenuItem::new("Reading the repo…").disabled(true));
                    };
                    menu = menu.item(
                        PopupMenuItem::new(if info.dirty == 0 {
                            "Nothing uncommitted".to_string()
                        } else {
                            format!("{} uncommitted", info.dirty)
                        })
                        .icon(bench_ui::icons::Ico::CircleDot.icon())
                        .disabled(true),
                    );
                    if !info.commits.is_empty() {
                        menu = menu.separator();
                        for c in info.commits {
                            let line = format!("{}  {} · {}", c.sha, c.subject, c.age);
                            menu = menu.item(
                                copy(app.clone(), bench_ui::clip(&line, 56), line).disabled(false),
                            );
                        }
                    }
                    if let Some(url) = info.remote {
                        menu = menu.separator().item(
                            copy(app.clone(), bench_ui::clip(&url, 56), url)
                                .icon(bench_ui::icons::Ico::Globe.icon()),
                        );
                    }
                    menu
                })
                .into_any_element(),
        )
    }

    /// Header strip: the session title, or the settings tabs.
    pub(crate) fn render_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let bar = h_flex()
            .id("header")
            .h(bench_ui::header_h(cx))
            .flex_none()
            .bg(theme.title_bar)
            .items_center()
            .border_b_1()
            .border_color(theme.border);

        // Documents: the header names the project they belong to; their tabs are the center's
        // own bar under it (as Zed's pane), not the window's title bar, which drags the window.
        if self.center == Center::Doc {
            let name = self.project_name(&self.explorer.project);
            return bar
                .pl_4()
                .pr_2()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        // The bars' one size, the status bar's: title, chips and buttons alike.
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(name),
                )
                .child(self.render_header_tail(cx))
                .into_any_element();
        }
        // Settings and the new project's page: their own close sits here.
        if !self.center.is_settings() {
            let title: SharedString = self
                .thread(cx)
                .id
                .as_ref()
                .and_then(|id| self.threads.list.iter().find(|s| &s.id == id))
                .map(|s| s.title.clone())
                .filter(|t| !t.is_empty())
                // A thread about a task list is named after it until it has a title of its own.
                .unwrap_or_else(|| "New thread".into())
                .into();
            return bar
                .pl_4()
                .pr_2()
                .gap_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        // The bars' one size, the status bar's: title, chips and buttons alike.
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(title),
                )
                // The thread's own meters, then its repo: what is this thread's, together.
                .children(self.active_thread.read(cx).render_meters(self.defaults.context_length, cx))
                .children(self.render_project_chip(cx))
                .child(self.render_header_tail(cx))
                .into_any_element();
        }

        let title: SharedString = "Settings".into();
        bar.child(
            div()
                .px_4()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
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
                .child(bench_ui::icons::Ico::X.icon().small())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.center = Center::Chat;
                    this.new_project.form = None;
                    cx.notify();
                })),
        )
        .into_any_element()
    }
}
