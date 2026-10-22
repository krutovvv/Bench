//! The conversation as one scrolling column: per turn the user message, folded steps
//! (each unfoldable), the answer, sources and hover actions; after the last turn, the
//! files this session changed.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::text::TextView;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{clip, clock, plural};
use crate::app::{HermesApp, tool_label};
use crate::history::host_of;
use crate::icons::Ico;
use crate::model::{Step, Turn};

impl HermesApp {
    /// New chat: optionally work in a git project, optionally in a branch of its own.
    fn render_project_choice(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let projects = self.known_projects();
        let chosen = self.new_project.clone();
        let label = chosen
            .as_deref()
            .map(|r| r.rsplit('/').next().unwrap_or(r).to_string())
            .unwrap_or_else(|| "No project".into());
        let view = cx.entity();
        let picker = Button::new("new-project")
            .ghost()
            .small()
            .icon(Ico::Folder.icon())
            .label(label)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu;
                let none = view.clone();
                menu = menu.item(
                    PopupMenuItem::new("No project")
                        .checked(chosen.is_none())
                        .on_click(move |_, _, cx| {
                            none.update(cx, |this, cx| {
                                this.new_project = None;
                                this.new_branch = false;
                                cx.notify();
                            })
                        }),
                );
                let browse = view.clone();
                menu = menu.item(PopupMenuItem::new("Choose folder…").on_click(
                    move |_, window, cx| {
                        browse.update(cx, |this, cx| this.choose_project_folder(window, cx))
                    },
                ));
                if !projects.is_empty() {
                    menu = menu.separator();
                }
                for root in &projects {
                    let (view, r) = (view.clone(), root.clone());
                    menu = menu.item(
                        PopupMenuItem::new(root.clone())
                            .checked(chosen.as_deref() == Some(root))
                            .on_click(move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.new_project = Some(r.clone());
                                    this.new_project_repo = true;
                                    cx.notify();
                                })
                            }),
                    );
                }
                menu
            });
        let on = self.new_branch;
        h_flex()
            .mt_4()
            .gap_3()
            .items_center()
            .text_sm()
            .child(picker)
            .when(self.path_entry, |d| {
                d.child(
                    div().w(px(320.)).child(
                        gpui_kit::component::input::Input::new(&self.project_path).small(),
                    ),
                )
            })
            .when(self.new_project.is_some() && self.new_project_repo, |d| {
                d.child(
                    h_flex()
                        .id("new-branch")
                        .gap_1p5()
                        .items_center()
                        .cursor_pointer()
                        .text_color(if on { theme.foreground } else { muted })
                        .hover(|s| s.text_color(theme.foreground))
                        .child(
                            div()
                                .size(px(14.))
                                .border_1()
                                .border_color(if on { theme.primary } else { theme.input })
                                .when(on, |d| d.bg(theme.primary)),
                        )
                        .child(Ico::GitBranch.icon().xsmall())
                        .child("In a new branch")
                        .tooltip(|window, cx| {
                            Tooltip::new("A branch of its own in a separate folder (git worktree): main stays untouched until you merge")
                                .build(window, cx)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.new_branch = !this.new_branch;
                            cx.notify();
                        })),
                )
            })
    }

    pub(super) fn render_chat(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        if self.turns.is_empty() && self.session.is_some() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    ShimmerText::new("Loading history…")
                        .text_sm()
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }
        if self.turns.is_empty() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("What are we working on?"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{} · {}",
                            self.target.label(),
                            self.current_profile()
                        )),
                )
                .child(self.render_project_choice(cx))
                .into_any_element();
        }

        // Follow the tail: stay pinned to the bottom until the user scrolls up.
        let max = self.chat_scroll.max_offset().y;
        let at_bottom = -self.chat_scroll.offset().y >= max - px(4.);
        if self.follow && !at_bottom && max == self.last_max {
            self.follow = false; // the user scrolled away
        }
        if at_bottom {
            self.follow = true;
        }
        if self.follow && !at_bottom {
            self.chat_scroll.scroll_to_bottom();
        }
        self.last_max = max;

        let rows: Vec<AnyElement> = (0..self.turns.len())
            .map(|i| self.render_turn(i, cx))
            .collect();
        let files = self.render_session_files(cx);

        div()
            .relative()
            .size_full()
            .child(
                v_flex()
                    .id("chat")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.chat_scroll)
                    .pb_4()
                    .children(rows)
                    .children(files),
            )
            .when(!self.follow, |d| {
                d.child(
                    h_flex()
                        .id("jump")
                        .absolute()
                        .bottom_3()
                        .right_4()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .bg(theme.background)
                        .border_1()
                        .border_color(theme.border)
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .hover(|s| s.text_color(theme.foreground))
                        .child(Ico::ChevronDown.icon().xsmall())
                        .child("Jump to latest")
                        .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx))),
                )
            })
            .into_any_element()
    }

    fn render_turn(&mut self, i: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let turn: &Turn = &self.turns[i];
        let last = i + 1 == self.turns.len();
        let live = self
            .live
            .as_ref()
            .filter(|_| last)
            .map(|l| (l.label.clone(), l.started.elapsed().as_secs()));
        let tools = turn.tool_count();
        let errors = turn
            .steps
            .iter()
            .filter(|s| matches!(s, Step::Tool { failed: true, .. }))
            .count();
        let has_steps = !turn.steps.is_empty();
        let expanded = turn.expanded;
        let group: SharedString = format!("turn{i}").into();

        // One-line summary; only the error count is red.
        let summary = (has_steps || live.is_some()).then(|| {
            let mut tail: Vec<String> = vec![];
            if tools > 0 {
                tail.push(plural(tools, "action", "actions", "actions"));
            }
            if let Some((_, s)) = &live {
                tail.push(clock(*s));
            }
            h_flex()
                .id(("steps", i))
                .gap_1p5()
                .items_center()
                .text_sm()
                .text_color(muted)
                .min_w_0()
                .when(has_steps, |d| {
                    d.cursor_pointer().hover(|s| s.text_color(theme.foreground))
                })
                .child(
                    (if expanded {
                        Ico::ChevronDown
                    } else {
                        Ico::ChevronRight
                    })
                    .icon()
                    .xsmall()
                    .when(!has_steps, |i| i.text_color(theme.transparent)),
                )
                .map(|d| match &live {
                    Some((label, _)) => d.child(
                        ShimmerText::new(label.clone())
                            .text_color(muted)
                            .max_w(px(560.)),
                    ),
                    None => d.child(div().flex_none().child("Done")),
                })
                .when(!tail.is_empty(), |d| {
                    d.child(div().flex_none().child(format!("· {}", tail.join(" · "))))
                })
                .when(errors > 0, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .text_color(theme.danger)
                            .child(format!("· {}", plural(errors, "error", "errors", "errors"))),
                    )
                })
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_turn(i, cx)))
        });

        let steps: Option<Div> = expanded.then(|| {
            let items: Vec<AnyElement> = (0..self.turns[i].steps.len())
                .map(|k| self.render_step(i, k, cx))
                .collect();
            v_flex()
                .ml(px(6.))
                .pl_4()
                .gap_1()
                .border_l_1()
                .border_color(theme.border)
                .text_sm()
                .children(items)
        });

        let turn: &Turn = &self.turns[i];
        let answer = (!turn.answer.trim().is_empty()).then(|| {
            TextView::markdown(
                SharedString::from(format!("answer-{i}")),
                turn.answer.clone(),
            )
        });
        let has_answer = !turn.answer.trim().is_empty();
        let time = match (turn.started, turn.finished) {
            (Some(a), Some(b)) if b > a => clock((b - a) as u64),
            _ => String::new(),
        };
        let when = turn.finished.or(turn.started).unwrap_or_default();
        let user = turn.user.clone();

        let sources = self.render_sources(i, cx);
        let actions = (live.is_none() && has_answer).then(|| {
            let playing = self.playing.as_ref().is_some_and(|(j, _)| *j == i);
            let speaking = self.speaking == Some(i);
            h_flex()
                .gap_0p5()
                .items_center()
                .opacity(0.)
                .group_hover(group.clone(), |s| s.opacity(1.))
                .when(!time.is_empty(), |d| {
                    d.child(
                        div()
                            .id(("time", i))
                            .px_1()
                            .text_xs()
                            .text_color(muted)
                            .child(time)
                            .tooltip(move |window, cx| {
                                Tooltip::new(format_when(when)).build(window, cx)
                            }),
                    )
                })
                .child(icon_button(("copy", i), Ico::Copy, "Copy", cx).on_click(
                    cx.listener(move |this, _, window, cx| this.copy_answer(i, window, cx)),
                ))
                .child(
                    icon_button(
                        ("speak", i),
                        if playing {
                            Ico::Square
                        } else if speaking {
                            Ico::Loader
                        } else {
                            Ico::Volume
                        },
                        if playing { "Stop" } else { "Read aloud" },
                        cx,
                    )
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.toggle_speak(i, window, cx)),
                    ),
                )
                .when(last, |d| {
                    d.child(
                        icon_button(("regen", i), Ico::RotateCcw, "Regenerate", cx)
                            .on_click(cx.listener(|this, _, _, cx| this.regenerate(cx))),
                    )
                })
        });

        v_flex()
            .id(("turn", i))
            .group(group)
            .w_full()
            .px_4()
            .py_3()
            .gap_3()
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .whitespace_normal()
                    .child(user),
            )
            .children(summary)
            .children(steps)
            .children(answer)
            .child(
                h_flex()
                    .min_h(px(22.))
                    .gap_2()
                    .items_start()
                    .child(div().flex_1().min_w_0().children(sources))
                    .children(actions),
            )
            .into_any_element()
    }

    /// One step line; click unfolds its details (reasoning text, tool args and output).
    fn render_step(&self, i: usize, k: usize, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let open = self.open_steps.contains(&(i, k));
        let (icon, color, label, details): (Ico, Hsla, String, Vec<(&str, String)>) =
            match &self.turns[i].steps[k] {
                Step::Tool {
                    name,
                    detail,
                    failed,
                    args,
                    output,
                    ..
                } => (
                    if *failed {
                        Ico::CircleAlert
                    } else {
                        Ico::Terminal
                    },
                    if *failed { theme.danger } else { muted },
                    clip(&tool_label(name, detail), 220),
                    [("Arguments", args.clone()), ("Output", output.clone())]
                        .into_iter()
                        .filter(|(_, v)| !v.trim().is_empty())
                        .collect(),
                ),
                Step::Note(text) => (
                    Ico::ChevronRight,
                    theme.foreground.opacity(0.75),
                    clip(text, 400),
                    vec![],
                ),
                Step::Thought(text) => (
                    Ico::Brain,
                    muted,
                    clip(text, 160),
                    vec![("Reasoning", text.clone())],
                ),
            };
        let expandable = !details.is_empty();
        v_flex()
            .child(
                h_flex()
                    .id(SharedString::from(format!("step-{i}-{k}")))
                    .gap_2()
                    .items_start()
                    .text_color(color)
                    .when(expandable, |d| {
                        d.cursor_pointer().hover(|s| s.text_color(theme.foreground))
                    })
                    .child(icon.icon().xsmall().mt(px(3.)))
                    .child(div().flex_1().min_w_0().child(label))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_step(i, k, cx))),
            )
            .when(open && expandable, |d| {
                d.children(details.into_iter().map(|(title, body)| {
                    v_flex()
                        .ml_5()
                        .mt_1()
                        .mb_1()
                        .child(div().text_xs().text_color(muted).child(title))
                        .child(
                            div()
                                .mt_0p5()
                                .p_2()
                                .bg(theme.secondary)
                                .border_1()
                                .border_color(theme.border)
                                .font_family("Menlo")
                                .text_xs()
                                .text_color(theme.foreground)
                                .whitespace_normal()
                                .child(body),
                        )
                }))
            })
            .into_any_element()
    }

    /// One chip per site; click lists its pages, each opens on its own.
    fn render_sources(&self, i: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let urls = &self.turns[i].sources;
        if urls.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let mut hosts: Vec<(String, Vec<String>)> = vec![];
        for u in urls {
            let h = host_of(u);
            match hosts.iter_mut().find(|(x, _)| *x == h) {
                Some((_, pages)) => pages.push(u.clone()),
                None => hosts.push((h, vec![u.clone()])),
            }
        }
        hosts.sort_by_key(|(_, p)| std::cmp::Reverse(p.len()));
        let open_host = self
            .open_source
            .as_ref()
            .filter(|(t, _)| *t == i)
            .map(|(_, h)| h.clone());
        let pages = open_host
            .as_ref()
            .and_then(|h| hosts.iter().find(|(x, _)| x == h))
            .map(|(_, p)| p.clone())
            .unwrap_or_default();

        let chips: Vec<AnyElement> = hosts
            .into_iter()
            .take(12)
            .enumerate()
            .map(|(k, (host, pages))| {
                let active = open_host.as_deref() == Some(&host);
                let single = (pages.len() == 1).then(|| pages[0].clone());
                let h = host.clone();
                h_flex()
                    .id(SharedString::from(format!("src-{i}-{k}")))
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .border_1()
                    .border_color(if active { theme.primary } else { theme.border })
                    .text_xs()
                    .text_color(if active {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
                    .child(Ico::Globe.icon().xsmall())
                    .child(host)
                    .when(pages.len() > 1, |d| {
                        d.child(div().opacity(0.6).child(pages.len().to_string()))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| match &single {
                        Some(url) => cx.open_url(url),
                        None => this.toggle_source(i, h.clone(), cx),
                    }))
                    .into_any_element()
            })
            .collect();

        let list = (!pages.is_empty()).then(|| {
            v_flex()
                .mt_1()
                .pl_2()
                .border_l_1()
                .border_color(theme.border)
                .children(pages.into_iter().enumerate().map(|(k, url)| {
                    let open = url.clone();
                    div()
                        .id(SharedString::from(format!("page-{i}-{k}")))
                        .text_xs()
                        .text_color(theme.link)
                        .truncate()
                        .cursor_pointer()
                        .hover(|s| s.underline())
                        .child(
                            url.trim_start_matches("https://")
                                .trim_start_matches("http://")
                                .to_string(),
                        )
                        .on_click(move |_, _, cx| cx.open_url(&open))
                }))
        });

        Some(
            v_flex()
                .child(h_flex().flex_wrap().gap_1().children(chips))
                .children(list)
                .into_any_element(),
        )
    }

    /// After the last turn: every file this session edited, with +/- counts.
    fn render_session_files(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let files = self.session_files();
        if files.is_empty() || self.live.is_some() {
            return None;
        }
        let theme = cx.theme().clone();
        let rows: Vec<AnyElement> = files
            .into_iter()
            .enumerate()
            .map(|(k, (path, diffs))| {
                let (add, del) = diffs
                    .iter()
                    .map(|d| crate::diff::stats(d))
                    .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
                let p = std::path::Path::new(&path);
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let dir = p
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let selected = self.file.as_ref().is_some_and(|f| f.path == path);
                let open = path.clone();
                h_flex()
                    .id(("file", k))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .items_center()
                    .text_sm()
                    .cursor_pointer()
                    .border_l_2()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.transparent
                    })
                    .when(selected, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(name)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(dir),
                    )
                    .when(add > 0, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.success)
                                .child(format!("+{add}")),
                        )
                    })
                    .when(del > 0, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.danger)
                                .child(format!("−{del}")),
                        )
                    })
                    .when(diffs.is_empty(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("written"),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_file(open.clone(), window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .mx_4()
                .mt_2()
                .border_1()
                .border_color(theme.border)
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .border_b_1()
                        .border_color(theme.border)
                        .child("CHANGED FILES"),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}

fn icon_button(id: impl Into<ElementId>, icon: Ico, tip: &'static str, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .p_1()
        .cursor_pointer()
        .text_color(theme.muted_foreground)
        .hover(|s| s.bg(theme.list_hover).text_color(theme.foreground))
        .child(icon.live(false))
        .tooltip(move |window, cx| Tooltip::new(tip).build(window, cx))
}

/// "21.10.2026 22:15" for a Unix timestamp, in local time.
fn format_when(ts: f64) -> String {
    let out = std::process::Command::new("date")
        .args(["-r", &(ts as i64).to_string(), "+%d.%m.%Y %H:%M"])
        .output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}
