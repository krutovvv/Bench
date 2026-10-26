//! What hangs around the conversation: an approval the agent waits for, its background
//! processes, the files the thread changed, the pages a piece of work touched, a form.

use ::thread::{Row, Step};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::work::{host_of, tool_urls};
use crate::app::BenchApp;
use crate::icons::Ico;
use crate::ui::media::plain_text;
use crate::ui::{clock, plural};

impl BenchApp {
    /// The agent waits: what it wants to run, and the answers it accepts.
    pub(crate) fn render_approval(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let req = self.approval.clone()?;
        let theme = cx.theme().clone();
        let label = |c: &str| match c {
            "once" => "Allow once",
            "session" => "Allow for this chat",
            "always" => "Always allow",
            "deny" => "Deny",
            _ => "OK",
        };
        let buttons = req.choices.iter().enumerate().map(|(k, c)| {
            let choice = c.clone();
            let b = Button::new(("approve", k)).small().label(label(c));
            let b = if k == 0 {
                b.primary()
            } else if c == "deny" {
                b.ghost()
            } else {
                b.outline()
            };
            b.on_click(cx.listener(move |this, _, _, cx| this.answer_approval(choice.clone(), cx)))
        });
        Some(
            v_flex()
                .ml(px(crate::ui::CHAT_TEXT))
                .mr(px(crate::ui::CHAT_RIGHT))
                .mt_2()
                .p_3()
                .gap_2()
                .border_1()
                .rounded_md()
                .border_color(theme.warning)
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_base()
                        .font_weight(FontWeight::MEDIUM)
                        .child(Ico::CircleAlert.icon().small().text_color(theme.warning))
                        .child(if req.description.is_empty() {
                            "The agent asks to run this".to_string()
                        } else {
                            req.description.clone()
                        }),
                )
                .when(!req.command.is_empty(), |d| {
                    d.child(
                        div()
                            .p_2()
                            .bg(theme.secondary)
                            .font_family(crate::theme::MONO)
                            .text_base()
                            .child(req.command.clone()),
                    )
                })
                .child(h_flex().gap_2().children(buttons))
                .into_any_element(),
        )
    }

    /// The background processes the agent started in this session, as a row of its own where the
    /// work began: running or finished, each with Stop (while it runs) and its latest output a
    /// click away.
    pub(crate) fn render_processes(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.processes.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let now = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0., |d| d.as_secs_f64());
        let running = self.processes.iter().filter(|p| p.running).count();
        let done = self.processes.len() - running;
        let open = self.processes_open;
        let mut bits = vec![];
        if running > 0 {
            bits.push(format!("{running} running"));
        }
        if done > 0 {
            bits.push(format!("{done} finished"));
        }
        let rows: Vec<AnyElement> = self
            .processes
            .iter()
            .filter(|_| open)
            .enumerate()
            .map(|(k, p)| {
                let failed = !p.running && p.exit_code.is_some_and(|c| c != 0);
                let elapsed = p
                    .started
                    .map(|t| (p.exited.unwrap_or(now) - t).max(0.) as u64)
                    .map(clock);
                let state = if p.running {
                    elapsed.unwrap_or_default()
                } else {
                    let code = p
                        .exit_code
                        .map_or("ended".to_string(), |c| format!("exit {c}"));
                    // A reason that only says it ended adds nothing to "exit N".
                    let reason = p.reason.as_deref().filter(|r| {
                        !r.is_empty() && !matches!(*r, "exited" | "exit" | "completed")
                    });
                    match (reason, elapsed) {
                        (Some(r), Some(e)) => format!("{code} · {r} · {e}"),
                        (_, Some(e)) => format!("{code} · {e}"),
                        _ => code,
                    }
                };
                let shown = self.process_open.as_deref() == Some(p.id.as_str());
                let (id_toggle, id_kill) = (p.id.clone(), p.id.clone());
                // "cd <project> && cargo build": the folder is the session's, the rest is news.
                let shown_cmd = match p.command.strip_prefix("cd ") {
                    Some(rest) => rest
                        .split_once(" && ")
                        .map_or(p.command.as_str(), |(_, cmd)| cmd),
                    None => p.command.as_str(),
                }
                .to_string();
                let full_cmd = p.command.clone();
                let tail: String = {
                    let lines: Vec<&str> = p.tail.trim_end().lines().collect();
                    lines[lines.len().saturating_sub(60)..].join("\n")
                };
                v_flex()
                    // Rules between the processes, not under the head.
                    .when(k > 0, |d| d.border_t_1().border_color(theme.border))
                    .child(
                        h_flex()
                            .id(("proc", k))
                            .px_2()
                            .py_1()
                            .gap_2()
                            .items_center()
                            .text_base()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(if p.running {
                                Ico::Loader.live(false)
                            } else if failed {
                                Ico::CircleAlert
                                    .icon()
                                    .xsmall()
                                    .text_color(theme.danger)
                                    .into_any_element()
                            } else {
                                Ico::CircleCheck
                                    .icon()
                                    .xsmall()
                                    .text_color(muted)
                                    .into_any_element()
                            })
                            .child(
                                div()
                                    .id(("proc-cmd", k))
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(crate::theme::MONO)
                                    .child(shown_cmd)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(full_cmd.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_color(if failed { theme.danger } else { muted })
                                    .child(state),
                            )
                            .when(p.running, |d| {
                                d.child(
                                    div()
                                        .id(("proc-stop", k))
                                        .flex_none()
                                        .px_1p5()
                                        .text_color(muted)
                                        .hover(|s| {
                                            s.bg(theme.danger).text_color(theme.danger_foreground)
                                        })
                                        .child("Stop")
                                        .tooltip(|window, cx| {
                                            Tooltip::new("Stop this process").build(window, cx)
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.kill_process(id_kill.clone(), cx)
                                        })),
                                )
                            })
                            .child(
                                if shown {
                                    Ico::ChevronDown
                                } else {
                                    Ico::ChevronRight
                                }
                                .icon()
                                .xsmall()
                                .text_color(muted),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_process_output(id_toggle.clone(), cx)
                            })),
                    )
                    .when(shown, |d| {
                        d.child(
                            div()
                                .mx_2()
                                .mb_2()
                                .p_2()
                                .bg(theme.secondary)
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .font_family(crate::theme::MONO)
                                .text_base()
                                .text_color(theme.foreground)
                                .child(if tail.trim().is_empty() {
                                    div()
                                        .text_color(muted)
                                        .child("No output yet")
                                        .into_any_element()
                                } else {
                                    plain_text(format!("proc-out-{k}"), &tail).into_any_element()
                                }),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let head = crate::ui::block_head(&theme, open)
            .id("procs-head")
            // As short as the thread's other small rows, not as tall as a block's head.
            .h(px(crate::ui::CHIP_H))
            .gap_1p5()
            .cursor_pointer()
            .hover(|s| s.text_color(theme.foreground))
            .child(
                if open {
                    Ico::ChevronDown
                } else {
                    Ico::ChevronRight
                }
                .icon()
                .xsmall(),
            )
            .child(Ico::Terminal.icon().xsmall())
            .child(
                div()
                    .flex_none()
                    .child(format!("BACKGROUND · {}", bits.join(" · "))),
            )
            .when(running > 0, |d| d.child(Ico::Loader.live(false)))
            .when(!open, |d| d.child(div().flex_1()))
            .on_click(cx.listener(|this, _, _, cx| {
                // Folding is the thread's own click: it must not pull the focus to the composer.
                this.click_claimed = true;
                this.processes_open = !this.processes_open;
                cx.notify();
            }));
        let frame = v_flex()
            .id("procs-block")
            .w_full()
            .min_w_0()
            .flex_none()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .overflow_hidden()
            .child(head)
            .when(open, |d| d.children(rows));
        // The agent's row, as its plan's: its avatar, the block in the text column.
        Some(
            h_flex()
                .w_full()
                .flex_none()
                .id("procs-row")
                .pl(px(crate::ui::CHAT_LEFT))
                .pr(px(crate::ui::CHAT_RIGHT))
                .py(px(crate::ui::TURN_Y))
                .gap(px(crate::ui::CHAT_TEXT
                    - crate::ui::CHAT_LEFT
                    - crate::ui::AVATAR))
                .items_start()
                .child(crate::ui::avatar(true, &theme))
                .child(div().flex_1().min_w_0().child(frame))
                .into_any_element(),
        )
    }

    /// After the last turn: every file this session edited, with +/- counts.
    pub(crate) fn render_session_files(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let files = self.session_files();
        if files.is_empty() || self.thread.running() {
            return None;
        }
        let theme = cx.theme().clone();
        let open = self.files_open;
        let count = files.len();
        let (all_add, all_del) = files
            .iter()
            .flat_map(|(_, diffs)| diffs.iter().map(|d| crate::diff::stats(d)))
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let rows: Vec<AnyElement> = files
            .into_iter()
            .filter(|_| open)
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
                let selected = false;
                let open = path.clone();
                h_flex()
                    .id(("file", k))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .items_center()
                    .text_base()
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
                            .text_base()
                            .text_color(theme.muted_foreground)
                            .truncate()
                            .child(dir),
                    )
                    .when(add > 0, |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.success)
                                .child(format!("+{add}")),
                        )
                    })
                    .when(del > 0, |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.danger)
                                .child(format!("−{del}")),
                        )
                    })
                    .when(diffs.is_empty(), |d| {
                        d.child(
                            div()
                                .text_base()
                                .text_color(theme.muted_foreground)
                                .child("written"),
                        )
                    })
                    // In Zed, the code editor beside Bench.
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let host = this.thread_host();
                        this.open_in_zed(&open, None, Some(host.as_str()), cx)
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .border_1()
                .rounded_md()
                .border_color(theme.border)
                // Folded by default: one line with the totals; a click lists the files.
                .child(
                    h_flex()
                        .id("files-head")
                        // As tall as a source chip (px_1p5 py_0p5 around text_xs).
                        .px_1p5()
                        .py_0p5()
                        .gap_1p5()
                        .items_center()
                        .text_base()
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .hover(|s| s.text_color(theme.foreground))
                        .when(open, |d| d.border_b_1().border_color(theme.border))
                        .child(
                            if open {
                                Ico::ChevronDown
                            } else {
                                Ico::ChevronRight
                            }
                            .icon()
                            .xsmall(),
                        )
                        .child(format!(
                            "CHANGED FILES · {}",
                            plural(count, "file", "files", "files")
                        ))
                        .when(all_add > 0, |d| {
                            d.child(div().text_color(theme.success).child(format!("+{all_add}")))
                        })
                        .when(all_del > 0, |d| {
                            d.child(div().text_color(theme.danger).child(format!("−{all_del}")))
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.files_open = !this.files_open;
                            cx.notify();
                        })),
                )
                .children(rows)
                .into_any_element(),
        )
    }

    /// The pages the work right above answer `k` touched: one chip per site.
    pub(crate) fn render_sources(&self, k: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Row::Work { key, steps, .. }) = k.checked_sub(1).map(|j| &self.tv.rows[j]) else {
            return None;
        };
        let mut urls = vec![];
        for s in steps {
            if let Step::Tool { name, args, .. } = s {
                tool_urls(name, args, &mut urls);
            }
        }
        if urls.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let mut hosts: Vec<(String, Vec<String>)> = vec![];
        for u in urls {
            let h = host_of(&u);
            match hosts.iter_mut().find(|(x, _)| *x == h) {
                Some((_, pages)) => pages.push(u),
                None => hosts.push((h, vec![u])),
            }
        }
        hosts.sort_by_key(|(_, p)| std::cmp::Reverse(p.len()));
        let open_host = self
            .open_source
            .as_ref()
            .filter(|(t, _)| t == key)
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
            .map(|(n, (host, pages))| {
                let active = open_host.as_deref() == Some(&host);
                let single = (pages.len() == 1).then(|| pages[0].clone());
                let (h, key) = (host.clone(), key.clone());
                h_flex()
                    .id(SharedString::from(format!("src-{key}-{n}")))
                    .gap_1()
                    .px_1p5()
                    .py_0p5()
                    .border_1()
                    .rounded_md()
                    .border_color(if active { theme.primary } else { theme.border })
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
                        None => {
                            this.click_claimed = true;
                            this.open_source = match &this.open_source {
                                Some((t, x)) if *t == key && *x == h => None,
                                _ => Some((key.clone(), h.clone())),
                            };
                            cx.notify();
                        }
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
                .children(pages.into_iter().enumerate().map(|(n, url)| {
                    let open = url.clone();
                    div()
                        .id(SharedString::from(format!("page-{key}-{n}")))
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

    /// Every file the thread's work edited or wrote: (path, its diffs).
    pub fn session_files(&self) -> Vec<(String, Vec<String>)> {
        let mut out: Vec<(String, Vec<String>)> = vec![];
        for r in &self.tv.rows {
            let Row::Work { steps, .. } = r else { continue };
            for s in steps {
                let Step::Tool {
                    name,
                    args,
                    diff,
                    failed: false,
                    output: Some(_),
                    ..
                } = s
                else {
                    continue;
                };
                if !matches!(name.as_str(), "edit" | "write") {
                    continue;
                }
                let Some(path) = args.get("path").and_then(|p| p.as_str()) else {
                    continue;
                };
                let slot = match out.iter().position(|(p, _)| p == path) {
                    Some(i) => &mut out[i].1,
                    None => {
                        out.push((path.to_string(), vec![]));
                        &mut out.last_mut().unwrap().1
                    }
                };
                slot.extend(diff.clone());
            }
        }
        out
    }

    /// A form opened from the thread (a new server or file host) that is not under an insert.
    pub(crate) fn render_tail_form(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let under_insert = self
            .form_after
            .as_ref()
            .is_some_and(|id| self.thread_inserts().iter().any(|i| &i.id == id));
        let form = if under_insert {
            None
        } else if self.files_remote.form {
            Some(self.render_host_form(cx))
        } else if self.ssh.form.is_some() {
            Some(self.render_server_form(cx))
        } else {
            None
        }?;
        Some(self.agent_row(form, cx))
    }
}
