//! The line under the composer, as Claude's under its field: on the left the project the thread
//! is in (a new thread's to choose: "New thread" in the sidebar starts one in the open thread's
//! project, and this says which) and the git repo it works in; on the right the thread's meters
//! (context, speed, fallback, `meters.rs`) and the terminals sent to the background.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex};
use gpui_kit::*;

use crate::app::model::ProjectKey;
use crate::thread::view::ThreadView;
use bench_ui::icons::Ico;

impl ThreadView {
    pub(crate) fn render_composer_foot(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(app) = self.app(cx) else {
            return div().into_any_element();
        };
        let meters = self.render_meters(app.defaults.context_length, cx);
        let project = self.render_project_pick(cx);
        let repo = self.render_repo_chip(cx);
        let terminals = self.render_background_terminals(cx);
        h_flex()
            .w_full()
            .min_w_0()
            .h(px(bench_ui::COMPOSER_FOOT_H))
            .gap_1()
            .items_center()
            .text_size(px(bench_ui::BAR_TEXT))
            .child(project)
            .children(repo)
            .child(div().flex_1())
            .children(terminals)
            .children(meters)
            .into_any_element()
    }

    /// The thread's project. A new thread (nothing sent yet) moves to the one chosen; a thread
    /// with messages stays where it is, and a choice starts a new thread there.
    fn render_project_pick(&self, cx: &mut Context<Self>) -> AnyElement {
        let app = self.app(cx).expect("checked by the caller");
        // A thread with messages is where the agent's list puts it; a new one, where it was made.
        let current = match &self.thread.id {
            Some(id) => app.project_of_thread(&self.thread.host, id),
            None => self.thread.project.clone(),
        };
        let name = if current.is_empty() || ProjectKey::parse(&current).is_host() {
            crate::app::LOCAL_HOST.to_string()
        } else {
            app.project_name(&current)
        };
        let mut choices: Vec<(String, String)> =
            vec![(ProjectKey::HOST.to_string(), crate::app::LOCAL_HOST.to_string())];
        choices.extend(app.places.projects.iter().map(|p| (p.id.clone(), p.name.clone())));
        let draft = self.thread.id.is_none();
        let view = cx.entity().downgrade();
        let app_handle = self.app.clone();
        Button::new("composer-project")
            .ghost()
            .small()
            .text_color(cx.theme().muted_foreground)
            .icon(Ico::Folder.icon().xsmall())
            .child(div().text_size(px(bench_ui::BAR_TEXT)).child(name))
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu.label(if draft { "Project" } else { "New thread in" });
                for (id, name) in choices.iter().cloned() {
                    let (view, app) = (view.clone(), app_handle.clone());
                    let checked = draft && (id == current || (current.is_empty() && id == ProjectKey::HOST));
                    menu = menu.item(PopupMenuItem::new(name).checked(checked).on_click(
                        move |_, window, cx| {
                            let id = id.clone();
                            if draft {
                                let _ = view.update(cx, |this, cx| {
                                    this.thread.project = id;
                                    cx.notify();
                                });
                            } else {
                                let _ = app.update(cx, |app, cx| {
                                    app.new_thread_here(cx);
                                    app.active_thread.update(cx, |v, cx| {
                                        v.thread.project = id;
                                        cx.notify();
                                    });
                                    app.focus_composer(window, cx);
                                });
                            }
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }

    /// "app · main": the thread works in a git repository (its folder's name and branch). A
    /// click opens what the repo is: uncommitted changes, the newest commits (a click copies
    /// the sha), its origin. Git itself stays the agent's: ask it to commit, or to undo.
    fn render_repo_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = self.thread.repo.clone()?;
        let app = self.app(cx)?;
        let label = if p.branch.is_empty() {
            p.name()
        } else {
            format!("{} · {}", p.name(), p.branch)
        };
        let dir = p.dir.clone();
        let info = app.git.info.get(&dir).cloned();
        if info.is_none() {
            // Read once, after this frame: the app is not to be changed while the view draws.
            let (at, handle, dir) = (self.thread_ref(), self.app.clone(), dir.clone());
            cx.defer(move |cx| {
                let _ = handle.update(cx, |app, cx| {
                    let target = app.thread_target(&at);
                    app.refresh_git(target, dir, cx);
                });
            });
        }
        let handle = self.app.clone();
        let copy = |app: WeakEntity<crate::app::BenchApp>, shown: String, what: String| {
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
                .text_color(cx.theme().muted_foreground)
                .icon(Ico::GitBranch.icon().xsmall())
                // A Button's `.label()` takes the control's size: the bars' size is set here.
                .child(div().text_size(px(bench_ui::BAR_TEXT)).child(label))
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
                        .icon(Ico::CircleDot.icon())
                        .disabled(true),
                    );
                    if !info.commits.is_empty() {
                        menu = menu.separator();
                        for c in info.commits {
                            let line = format!("{}  {} · {}", c.sha, c.subject, c.age);
                            menu = menu.item(copy(handle.clone(), bench_ui::clip(&line, 56), line));
                        }
                    }
                    if let Some(url) = info.remote {
                        menu = menu.separator().item(
                            copy(handle.clone(), bench_ui::clip(&url, 56), url)
                                .icon(Ico::Globe.icon()),
                        );
                    }
                    menu
                })
                .into_any_element(),
        )
    }

    /// Terminals sent to the background, while there are any: a menu where each brings it back
    /// into its thread, and Stop all ends them.
    fn render_background_terminals(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let app = self.app(cx)?;
        let ids: Vec<_> = app.inserts.ssh.tabs.iter().filter(|t| t.background).map(|t| t.id).collect();
        if ids.is_empty() {
            return None;
        }
        let handle = self.app.clone();
        Some(
            Button::new("bg-terminals")
                .ghost()
                .small()
                .icon(Ico::Terminal.icon().xsmall())
                .text_color(cx.theme().primary)
                .child(div().text_size(px(bench_ui::BAR_TEXT)).child(ids.len().to_string()))
                .tooltip("Running terminals")
                .dropdown_menu(move |menu, _, _| {
                    let mut menu = menu.label("Running terminals").separator();
                    for (k, id) in ids.iter().copied().enumerate() {
                        let app = handle.clone();
                        menu = menu.item(PopupMenuItem::new(format!("Terminal {}", k + 1)).on_click(
                            move |_, _, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    if let Some(t) = this.inserts.ssh.tabs.iter_mut().find(|t| t.id == id) {
                                        t.background = false;
                                        t.collapsed = false;
                                    }
                                    cx.notify();
                                });
                            },
                        ));
                    }
                    let app = handle.clone();
                    menu.separator().item(PopupMenuItem::new("Stop all").on_click(move |_, _, cx| {
                        let _ = app.update(cx, |this, cx| {
                            for t in this.inserts.ssh.tabs.iter().filter(|t| t.background) {
                                t.session.kill();
                            }
                            this.inserts.ssh.tabs.retain(|t| !t.background);
                            cx.notify();
                        });
                    }))
                })
                .into_any_element(),
        )
    }
}
