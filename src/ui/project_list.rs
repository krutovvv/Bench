//! The new project's page (the sidebar's "+"), and a project's environment as rows in its thread.

use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// The new project's page (the sidebar's "+"), laid out like Settings: what it is called,
    /// where its agent runs (this Mac or a server), and on this Mac a folder if it has one.
    pub(super) fn render_project_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(form) = self.project_form.as_ref() else {
            return div().into_any_element();
        };
        let (name, host, folder) = (form.name.clone(), form.host.clone(), form.folder.clone());
        let (new_host, new_user, new_port, new_key) = (
            form.new_host.clone(),
            form.new_user.clone(),
            form.new_port.clone(),
            form.new_key.clone(),
        );
        let mut page: Vec<AnyElement> = vec![
            div()
                .pt_6()
                .text_base()
                .text_color(muted)
                .child("A project is what you work on: a client, a site, a codebase. Its threads share its environment (soul, MCP servers, skills, prompts, Telegram, schedule); you set that up in its environment thread once it exists.")
                .into_any_element(),
            self.pv_heading("PROJECT", cx).into_any_element(),
            self.pv_row("Name", "How it shows in the sidebar", self.pv_input(&name), cx).into_any_element(),
        ];
        // Where it runs: this Mac, or a server (its agent there keeps the threads, skills, MCP).
        let on_server = host.is_some();
        let place = super::tools::segmented_tall(
            "project-place",
            &["This Mac", "A server"],
            on_server as usize,
            cx,
            |this, k, _, cx| {
                if let Some(f) = this.project_form.as_mut() {
                    f.host = if k == 0 {
                        None
                    } else {
                        Some(
                            this.ssh
                                .servers
                                .first()
                                .map(|s| s.alias.clone())
                                .unwrap_or_default(),
                        )
                    };
                }
                cx.notify();
            },
        );
        page.push(
            self.pv_row(
                "Runs on",
                "Where its agent works and keeps its threads",
                h_flex().child(place).into_any_element(),
                cx,
            )
            .into_any_element(),
        );
        if let Some(chosen) = host.clone() {
            // The servers from ~/.ssh/config, and a new one typed in.
            let mut labels: Vec<String> =
                self.ssh.servers.iter().map(|s| s.alias.clone()).collect();
            let at = labels
                .iter()
                .position(|a| *a == chosen)
                .unwrap_or(labels.len());
            labels.push("New server…".into());
            let aliases = labels.clone();
            let pick = super::tools::segmented_tall_owned(
                "project-server",
                labels,
                at,
                cx,
                move |this, k, _, cx| {
                    if let Some(f) = this.project_form.as_mut() {
                        f.host = Some(if k + 1 == aliases.len() {
                            String::new()
                        } else {
                            aliases[k].clone()
                        });
                    }
                    cx.notify();
                },
            );
            page.push(
                self.pv_row(
                    "Server",
                    "A host from ~/.ssh/config, or a new one",
                    h_flex().flex_wrap().child(pick).into_any_element(),
                    cx,
                )
                .into_any_element(),
            );
            if chosen.is_empty() {
                page.push(
                    self.pv_row("Address", "Its IP or domain", self.pv_input(&new_host), cx)
                        .into_any_element(),
                );
                page.push(
                    self.pv_row("Login", "The user on it", self.pv_input(&new_user), cx)
                        .into_any_element(),
                );
                page.push(
                    self.pv_row("Port", "Empty: 22", self.pv_input(&new_port), cx)
                        .into_any_element(),
                );
                page.push(
                    self.pv_row("Key file", "Empty: ssh-agent", self.pv_input(&new_key), cx)
                        .into_any_element(),
                );
            }
            page.push(
                div()
                    .pt_2()
                    .text_sm()
                    .text_color(muted)
                    .child("Bench installs and starts the agent there over SSH (it needs Node.js 22.18+). The project's threads, skills and MCP servers live on that server.")
                    .into_any_element(),
            );
        } else {
            let has = folder.is_some();
            let control = h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_base()
                        .text_color(if has { theme.foreground } else { muted })
                        .child(folder.clone().unwrap_or_else(|| "None".into())),
                )
                .child(
                    super::form_action(
                        "project-folder",
                        if has { "Change…" } else { "Choose…" },
                        false,
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.pick_project_codebase(window, cx)),
                    ),
                )
                .when(has, |d| {
                    d.child(
                        super::form_action("project-folder-clear", "Clear", false, cx).on_click(
                            cx.listener(|this, _, _, cx| {
                                if let Some(f) = this.project_form.as_mut() {
                                    f.folder = None;
                                }
                                cx.notify();
                            }),
                        ),
                    )
                })
                .into_any_element();
            page.push(
                self.pv_row(
                    "Folder",
                    "Optional: a codebase its threads work in (opens in Zed)",
                    control,
                    cx,
                )
                .into_any_element(),
            );
        }
        page.push(
            h_flex()
                .pt_4()
                .gap_2()
                .child(
                    super::form_action("project-save", "Create project", true, cx)
                        .on_click(cx.listener(|this, _, window, cx| this.save_project(window, cx))),
                )
                .child(
                    super::form_action("project-cancel", "Cancel", false, cx).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.project_form = None;
                            this.panel = crate::app::Panel::Chat;
                            cx.notify();
                        }),
                    ),
                )
                .into_any_element(),
        );
        v_flex()
            .id("new-project")
            .size_full()
            .overflow_y_scroll()
            .px_4()
            .pb_6()
            .child(v_flex().max_w(px(820.)).children(page))
            .into_any_element()
    }
}

impl BenchApp {
    /// A project's environment at the top of its thread, as a document of rows: its name, soul,
    /// Telegram bot, MCP servers, skills, prompts and accesses. A row opens in place as a code
    /// block (line numbers, highlighting) with Save / Revert under it, and closes again; "+ Add"
    /// starts the request for the agent in the composer.
    /// A project's environment as rows in blocks (the task list's look: a line between rows, a
    /// row lit under the pointer); a row's click opens its card under the block, as the agent's
    /// answer (`render_env_card`), where it is edited, saved or removed.
    pub(super) fn render_env_sections(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<(&'static str, AnyElement)> {
        let _ = window;
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return vec![(
                "",
                div().text_color(muted).child("Loading…").into_any_element(),
            )];
        };
        let remote = self
            .projects
            .iter()
            .any(|p| p.id == e.id && p.host.is_some());
        let general = e.id == "general";
        let card = e.card.clone();
        // A quiet text action in a head: muted, the foreground on hover.
        let act = |id: SharedString, label: &'static str| {
            div()
                .id(id)
                .flex_none()
                .cursor_pointer()
                .text_color(muted)
                .hover(|s| s.text_color(theme.foreground))
                .child(label)
        };
        // "+ Add": a new one's card (paste it, or ask the agent from there); accesses are the
        // agent's to add.
        let add =
            |id: &'static str, kind: &'static str, ask: &'static str, cx: &mut Context<Self>| {
                act(id.into(), "+ Add").on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    if kind.is_empty() {
                        this.ask_in_composer(ask, window, cx)
                    } else {
                        this.env_card(kind, "new".into(), window, cx)
                    }
                }))
            };
        // A row: its name, a one-line note, its actions on hover; lit when its card is open.
        let row_with = |kind: &'static str,
                        name: String,
                        label: String,
                        note: String,
                        bad: bool,
                        first: bool,
                        actions: Vec<AnyElement>,
                        cx: &mut Context<Self>| {
            let note = note.split_whitespace().collect::<Vec<_>>().join(" ");
            let on = card.as_ref().is_some_and(|(k, n)| *k == kind && *n == name);
            let group = SharedString::from(format!("env-g-{kind}-{name}"));
            h_flex()
                .id(SharedString::from(format!("env-{kind}-{name}")))
                .group(group.clone())
                .px_3()
                .py_1p5()
                .gap_2()
                .cursor_pointer()
                .when(!first, |d| {
                    d.border_t_1().border_color(theme.border.opacity(0.5))
                })
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover))
                .child(div().flex_none().child(label))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(if bad { theme.danger } else { muted })
                        .child(note),
                )
                .children(actions.into_iter().map(|a| {
                    div()
                        .flex_none()
                        .opacity(0.)
                        .group_hover(group.clone(), |s| s.opacity(1.))
                        .child(a)
                }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_card(kind, name.clone(), window, cx)
                }))
                .into_any_element()
        };
        let row = |kind: &'static str,
                   name: String,
                   label: String,
                   note: String,
                   bad: bool,
                   first: bool,
                   cx: &mut Context<Self>| {
            row_with(kind, name, label, note, bad, first, vec![], cx)
        };
        let empty = |text: &'static str| {
            div()
                .px_3()
                .py_2()
                .text_color(muted)
                .child(text)
                .into_any_element()
        };
        let folded = e.folded.clone();
        let section = |title: &'static str,
                       extra: Option<AnyElement>,
                       rows: Vec<AnyElement>,
                       cx: &mut Context<Self>| {
            let shut = folded.contains(title);
            let head = super::block_head(&theme, !shut)
                .id(SharedString::from(format!("env-sec-{title}")))
                .cursor_pointer()
                .child(
                    if shut {
                        crate::icons::Ico::ChevronRight
                    } else {
                        crate::icons::Ico::ChevronDown
                    }
                    .icon()
                    .xsmall(),
                )
                .child(div().text_color(theme.foreground).child(title))
                .children(extra)
                .child(div().flex_1())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.click_claimed = true;
                    if let Some(e) = this.project_env.as_mut() {
                        if !e.folded.remove(title) {
                            e.folded.insert(title);
                        }
                    }
                    cx.notify();
                }));
            v_flex()
                .w_full()
                .border_1()
                .rounded_md()
                .border_color(theme.border)
                .overflow_hidden()
                .bg(theme.background)
                .child(head)
                .when(!shut, |d| d.children(rows))
                .into_any_element()
        };
        let mut blocks = vec![];

        // The project: its name, soul, Telegram bot.
        let mut rows = vec![];
        if !general {
            rows.push(row(
                "name",
                String::new(),
                "Name".into(),
                e.name.read(cx).value().to_string(),
                false,
                true,
                cx,
            ));
        }
        let soul_text = e.soul.read(cx).value().to_string();
        let first = soul_text
            .lines()
            .map(|l| l.trim_start_matches('#').trim())
            .find(|l| !l.is_empty())
            .unwrap_or("not described yet")
            .to_string();
        rows.push(row(
            "soul",
            String::new(),
            "Soul".into(),
            first,
            false,
            general,
            cx,
        ));
        let tg_note = match &e.telegram {
            Some((name, users, token)) if !token.is_empty() => format!(
                "{} · {} may write",
                if name.is_empty() {
                    "a bot".to_string()
                } else {
                    format!("@{name}")
                },
                users.len()
            ),
            _ => "no bot of its own".into(),
        };
        rows.push(row(
            "telegram",
            String::new(),
            "Telegram".into(),
            tg_note,
            false,
            false,
            cx,
        ));
        blocks.push((
            if general { "Host" } else { "Project" },
            section(if general { "Host" } else { "Project" }, None, rows, cx),
        ));

        // How it runs: how hard it thinks, what it may run without asking. Once for the project,
        // not per chat — its threads start on these. The model is one for every project (the gear).
        {
            let approval = match e.approvals.as_str() {
                "off" => "off — it asks for nothing",
                "manual" => "manual — it asks for everything",
                "" => "—",
                m => m,
            };
            let dash = |v: &str| {
                if v.is_empty() {
                    "—".to_string()
                } else {
                    v.to_string()
                }
            };
            let rows = vec![
                row(
                    "model",
                    "reasoning".into(),
                    "Reasoning".into(),
                    dash(&e.effort),
                    false,
                    false,
                    cx,
                ),
                row(
                    "model",
                    "approvals".into(),
                    "Approvals".into(),
                    approval.to_string(),
                    false,
                    false,
                    cx,
                ),
            ];
            blocks.push(("Model", section("Model", None, rows, cx)));
        }

        // MCP servers.
        let mut rows = vec![];
        if e.mcp.is_empty() {
            rows.push(empty("None of its own: the general ones work here too."));
        }
        for (k, (name, kind, enabled, connected, tools, error, _)) in
            e.mcp.clone().into_iter().enumerate()
        {
            let note = match (enabled, &error, connected) {
                (false, _, _) => format!("{kind} · off"),
                (_, Some(err), _) => format!("{kind} · {err}"),
                (_, _, true) => format!("{kind} · {tools} tools"),
                _ => format!("{kind} · connects with the project's first thread"),
            };
            rows.push(row(
                "mcp",
                name.clone(),
                name,
                note,
                error.is_some() && enabled,
                k == 0,
                cx,
            ));
        }
        let extra = add("env-mcp-add", "mcp", "", cx).into_any_element();
        blocks.push(("MCP servers", section("MCP servers", Some(extra), rows, cx)));

        // Skills: every one its threads can use, by where it lives; Finder on a row's hover.
        let mut rows = vec![];
        if e.skills.is_empty() {
            rows.push(empty("No skills yet: + Add writes one for this project."));
        }
        let groups: [(&str, &str); 3] = [
            ("project", "This project"),
            ("repo", "Its folder · .agents/skills, .claude/skills"),
            ("general", if general { "This Mac" } else { "General" }),
        ];
        for (scope, label) in groups {
            let skills: Vec<_> = e
                .skills
                .iter()
                .filter(|k| k.scope == scope)
                .cloned()
                .collect();
            if skills.is_empty() {
                continue;
            }
            rows.push(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .when(!rows.is_empty(), |d| {
                        d.border_t_1().border_color(theme.border.opacity(0.5))
                    })
                    .text_xs()
                    .text_color(muted)
                    .child(label)
                    .into_any_element(),
            );
            for k in skills {
                let finder = (!remote).then(|| {
                    let dir = k.dir.clone();
                    super::inserts::icon_button(
                        &format!("env-skill-finder-{}", k.name),
                        crate::icons::Ico::FolderOpen,
                        "Show in Finder",
                        &theme,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        cx.stop_propagation();
                        this.reveal_in_finder(&dir, window, cx)
                    }))
                    .into_any_element()
                });
                rows.push(row_with(
                    "skill",
                    k.name.clone(),
                    k.name,
                    k.description,
                    false,
                    false,
                    finder.into_iter().collect(),
                    cx,
                ));
            }
        }
        let extra = add("env-skill-add", "skill", "", cx).into_any_element();
        blocks.push(("Skills", section("Skills", Some(extra), rows, cx)));

        // Prompts (@ in the composer).
        let mut rows = vec![];
        if e.prompts.is_empty() {
            rows.push(empty(
                "None of its own: @ in the composer offers the general ones.",
            ));
        }
        for (k, name) in e.prompts.clone().into_iter().enumerate() {
            rows.push(row(
                "prompt",
                name.clone(),
                name,
                String::new(),
                false,
                k == 0,
                cx,
            ));
        }
        let extra = add("env-prompt-add", "prompt", "", cx).into_any_element();
        blocks.push(("Prompts", section("Prompts", Some(extra), rows, cx)));

        // Schedule: what runs on its own, each run a thread of the project.
        let mut rows = vec![];
        if e.cron.is_empty() {
            rows.push(empty("Nothing runs on its own yet: a job runs its prompt in a new thread of the project, on a schedule."));
        }
        for (k, j) in e.cron.clone().into_iter().enumerate() {
            let next = j
                .next_run
                .as_deref()
                .map(when)
                .unwrap_or_else(|| "no next run".into());
            let mut note = if j.enabled {
                format!("{} · next {next}", j.display)
            } else {
                format!("{} · paused", j.display)
            };
            if let Some(st) = &j.last_status {
                note.push_str(&format!(" · last: {st}"));
            }
            let failed = j.last_status.as_deref().is_some_and(|s| s != "ok");
            rows.push(row(
                "cron",
                j.id.clone(),
                j.name.clone(),
                note,
                failed,
                k == 0,
                cx,
            ));
        }
        let new_job = act("env-cron-add".into(), "+ New job")
            .on_click(cx.listener(|this, _, window, cx| {
                this.click_claimed = true;
                cx.stop_propagation();
                this.env_card("cron", "new".into(), window, cx)
            }))
            .into_any_element();
        blocks.push(("Schedule", section("Schedule", Some(new_job), rows, cx)));

        // Accesses (a remote project's are its server's own).
        if !remote {
            let mut rows = vec![];
            if e.hosts.is_empty() {
                rows.push(empty(
                    "No servers or file hosts yet: /ssh and /ftp in its threads offer only these.",
                ));
            }
            for (k, (hid, name, kind, at)) in e.hosts.clone().into_iter().enumerate() {
                rows.push(row(
                    "host",
                    hid,
                    name,
                    format!("{kind} · {at}"),
                    false,
                    k == 0,
                    cx,
                ));
            }
            let extra =
                add("env-host-add", "", "Add to this project the access: ", cx).into_any_element();
            blocks.push(("Accesses", section("Accesses", Some(extra), rows, cx)));
        }

        if let Some(err) = e.error.clone() {
            blocks.push((
                "",
                div().text_color(theme.danger).child(err).into_any_element(),
            ));
        }
        blocks
    }

    /// The environment's section a card belongs to (it shows under it).
    pub(super) fn env_card_section(&self) -> Option<&'static str> {
        let e = self.project_env.as_ref()?;
        let general = e.id == "general";
        Some(match e.card.as_ref()?.0 {
            "name" | "soul" | "telegram" => {
                if general {
                    "Host"
                } else {
                    "Project"
                }
            }
            "model" => "Model",
            "mcp" => "MCP servers",
            "skill" => "Skills",
            "prompt" => "Prompts",
            "cron" => "Schedule",
            _ => "Accesses",
        })
    }

    /// The open row's card, under the environment block as the agent's answer: its fields, and in
    /// its head Save (live once something changed), Revert, its own actions, Remove (twice) and
    /// Close.
    pub(super) fn render_env_card(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let e = self.project_env.as_ref()?;
        let (kind, name) = e.card.clone()?;
        let act = |id: String, label: String, color: Hsla| {
            div()
                .id(SharedString::from(id))
                .flex_none()
                .cursor_pointer()
                .text_color(color)
                .hover(|s| s.opacity(0.8))
                .child(label)
        };
        let save = |what: &'static str, dirty: bool, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("env-card-save-{what}")))
                .flex_none()
                .px_2()
                .rounded_md()
                .when(dirty, |d| {
                    d.cursor_pointer()
                        .bg(theme.primary)
                        .text_color(theme.primary_foreground)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.click_claimed = true;
                            this.env_save(what, window, cx)
                        }))
                })
                .when(!dirty, |d| d.text_color(muted))
                .child("Save")
                .into_any_element()
        };
        let revert = |what: &'static str, dirty: bool, cx: &mut Context<Self>| {
            dirty.then(|| {
                act(format!("env-card-revert-{what}"), "Revert".into(), muted)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_revert(what, window, cx)
                    }))
                    .into_any_element()
            })
        };
        let armed = e.armed.clone();
        let remove =
            |what: &'static str, target: String, label: &'static str, cx: &mut Context<Self>| {
                let on = armed.as_deref() == Some(format!("{what}:{target}").as_str());
                act(
                    format!("env-card-rm-{what}"),
                    if on {
                        format!("{label}?")
                    } else {
                        label.to_string()
                    },
                    theme.danger,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_remove(what, target.clone(), window, cx)
                }))
                .into_any_element()
            };
        let pad = |d: Div| d.px_3().py_2().gap_2().into_any_element();
        let mut actions: Vec<AnyElement> = vec![];
        // Cards that follow this one (a skill's open file).
        let mut follow: Vec<AnyElement> = vec![];
        let (title, body): (String, AnyElement) = match kind {
            "name" => {
                actions.push(save("name", e.name_dirty, cx));
                actions.extend(revert("name", e.name_dirty, cx));
                actions.push(remove("project", String::new(), "Remove the project", cx));
                (
                    "Name".into(),
                    pad(v_flex()
                        .child(gpui_kit::component::input::Input::new(&e.name))
                        .child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("Removing the project keeps its threads."),
                        )),
                )
            }
            "soul" => {
                actions.push(save("soul", e.soul_dirty, cx));
                actions.extend(revert("soul", e.soul_dirty, cx));
                (
                    "Soul: what the project is, how to work in it".into(),
                    code(&e.soul, cx),
                )
            }
            "telegram" => {
                actions.push(save("telegram", e.tg_dirty, cx));
                actions.extend(revert("telegram", e.tg_dirty, cx));
                (
                    "Telegram".into(),
                    pad(v_flex()
                        .child(gpui_kit::component::input::Input::new(&e.tg_token))
                        .child(gpui_kit::component::input::Input::new(&e.tg_users))
                        .child(div().text_sm().text_color(muted).child("Its chats become this project's threads, with its soul, skills and servers."))),
                )
            }
            "mcp" | "skill" | "prompt" if name == "new" => {
                let ask = act("env-card-ask".into(), "Ask the agent instead".into(), muted)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        let text = match kind {
                            "mcp" => "Add an MCP server to this project: ",
                            "skill" => "Write a skill for this project: ",
                            _ => "Write a prompt for this project: ",
                        };
                        this.ask_in_composer(text, window, cx)
                    }))
                    .into_any_element();
                actions.push(ask);
                actions.push(save(kind, true, cx));
                let (title, editor, named) = match kind {
                    "mcp" => (
                        "New MCP server: its name, and its config (Pi's format: {\"url\", \"headers\"} or {\"command\", \"args\", \"env\"})",
                        &e.mcp_json,
                        true,
                    ),
                    "skill" => (
                        "New skill: paste its SKILL.md (its front matter names it)",
                        &e.skill_text,
                        false,
                    ),
                    _ => ("New prompt: its name, and its text", &e.prompt_text, true),
                };
                let body = v_flex()
                    .when(named, |d| {
                        d.child(
                            div()
                                .px_3()
                                .pt_2()
                                .child(gpui_kit::component::input::Input::new(&e.new_name)),
                        )
                    })
                    .child(code(editor, cx))
                    .into_any_element();
                (title.to_string(), body)
            }
            "mcp" => {
                if !e.mcp.iter().any(|m| m.0 == name) {
                    return None;
                }
                if e.mcp_auth.contains(&name) {
                    let n = name.clone();
                    actions.push(
                        act("env-card-auth".into(), "Authorize".into(), theme.primary)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                this.env_authorize(n.clone(), window, cx)
                            }))
                            .into_any_element(),
                    );
                }
                actions.push(save("mcp", e.mcp_dirty, cx));
                actions.extend(revert("mcp", e.mcp_dirty, cx));
                actions.push(remove("mcp", name.clone(), "Remove", cx));
                (format!("MCP server · {name}"), code(&e.mcp_json, cx))
            }
            // A skill is a folder: its tree here, a new file's or folder's name and the open
            // file as the next cards.
            "skill" => {
                let k = e.skills.iter().find(|k| k.name == name)?.clone();
                let general = e.id == "general";
                let remote = self
                    .projects
                    .iter()
                    .any(|p| p.id == e.id && p.host.is_some());
                let folder = e.skill_folder.clone();
                for (what, ico, tip) in [
                    ("file", crate::icons::Ico::FilePlus, "New file"),
                    ("folder", crate::icons::Ico::FolderPlus, "New folder"),
                ] {
                    let folder = folder.clone();
                    actions.push(
                        super::inserts::icon_button(
                            &format!("env-skill-new-{what}"),
                            ico,
                            tip,
                            &theme,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.click_claimed = true;
                            this.skill_form(what, folder.clone(), window, cx)
                        }))
                        .into_any_element(),
                    );
                }
                if !remote {
                    let dir = k.dir.clone();
                    actions.push(
                        super::inserts::icon_button(
                            "env-skill-finder",
                            crate::icons::Ico::FolderOpen,
                            "Show in Finder",
                            &theme,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.click_claimed = true;
                            this.reveal_in_finder(&dir, window, cx)
                        }))
                        .into_any_element(),
                    );
                }
                // Only a skill of this environment's own goes as a whole; the folder's and the
                // general ones are removed where they live.
                if k.scope == "project" || (general && k.scope == "general") {
                    actions.push(remove("skill", name.clone(), "Remove", cx));
                }
                let (tree, form, file) = self.skill_parts(&k, cx);
                follow.extend(form);
                follow.extend(file);
                (format!("Skill · {name}"), tree)
            }
            "prompt" => {
                if !e.prompts.contains(&name) {
                    return None;
                }
                actions.push(save("prompt", e.prompt_dirty, cx));
                actions.extend(revert("prompt", e.prompt_dirty, cx));
                actions.push(remove("prompt", name.clone(), "Remove", cx));
                (format!("Prompt · {name}"), code(&e.prompt_text, cx))
            }
            "cron" => {
                let job = e.cron.iter().find(|j| j.id == name).cloned();
                if name != "new" && job.is_none() {
                    return None;
                }
                if let Some(j) = &job {
                    let (run, pause, enabled) = (j.id.clone(), j.id.clone(), j.enabled);
                    actions.push(
                        act("env-card-run".into(), "Run now".into(), muted)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                this.env_cron(run.clone(), "trigger", window, cx)
                            }))
                            .into_any_element(),
                    );
                    actions.push(
                        act(
                            "env-card-pause".into(),
                            if enabled {
                                "Pause".into()
                            } else {
                                "Resume".into()
                            },
                            muted,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.click_claimed = true;
                            this.env_cron(
                                pause.clone(),
                                if enabled { "pause" } else { "resume" },
                                window,
                                cx,
                            )
                        }))
                        .into_any_element(),
                    );
                }
                actions.push(save("cron", e.cron_dirty, cx));
                actions.extend(revert("cron", e.cron_dirty, cx));
                if job.is_some() {
                    actions.push(remove("cron", name.clone(), "Remove", cx));
                }
                let last = job
                    .as_ref()
                    .filter(|j| !j.last_output.trim().is_empty())
                    .map(|j| {
                        let at = j.last_run.as_deref().map(when).unwrap_or_default();
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(format!("Last run {at}: {}", j.last_output.trim()))
                    });
                (
                    job.as_ref()
                        .map(|j| format!("Job · {}", j.name))
                        .unwrap_or_else(|| "New job".into()),
                    pad(v_flex()
                        .child(gpui_kit::component::input::Input::new(&e.cron_name))
                        .child(gpui_kit::component::input::Input::new(&e.cron_schedule))
                        .child(
                            div()
                                .border_1()
                                .rounded_md()
                                .border_color(theme.border)
                                .child(code(&e.cron_prompt, cx)),
                        )
                        .children(last)),
                )
            }
            "host" => {
                let (hid, hname, hkind, at) = e.hosts.iter().find(|h| h.0 == name).cloned()?;
                actions.push(remove("host", hid, "Remove from the project", cx));
                (
                    format!("Access · {hname}"),
                    pad(v_flex().child(div().text_color(muted).child(format!("{hkind} · {at}")))),
                )
            }
            // A choice, not a text: what it can be, the current one marked; a click sets it for
            // the project at once (a choice has no draft to save).
            "model" => {
                let kind: &'static str = match name.as_str() {
                    "reasoning" => "reasoning",
                    _ => "approvals",
                };
                let (cur, opts): (String, &[&str]) = match kind {
                    "reasoning" => (e.effort.clone(), crate::model::EFFORTS),
                    _ => (e.approvals.clone(), crate::model::APPROVAL_MODES),
                };
                let mut items: Vec<AnyElement> = vec![];
                for value in opts.iter().map(|v| v.to_string()) {
                    let on = value == cur;
                    let (id, label) = (
                        SharedString::from(format!("env-run-{name}-{value}")),
                        value.clone(),
                    );
                    items.push(
                        h_flex()
                            .id(id)
                            .px_3()
                            .py_1p5()
                            .gap_2()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.list_hover))
                            .child(div().flex_1().min_w_0().truncate().child(label))
                            .when(on, |d| {
                                d.child(
                                    crate::icons::Ico::Check
                                        .icon()
                                        .xsmall()
                                        .text_color(theme.foreground),
                                )
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                cx.stop_propagation();
                                this.env_set_run(kind, value.clone(), window, cx)
                            }))
                            .into_any_element(),
                    );
                }
                let (title, note) = match kind {
                    "reasoning" => (
                        "Reasoning",
                        "How hard it thinks. A new thread of the project starts on it.",
                    ),
                    _ => (
                        "Approvals",
                        "What it may run without asking: off — nothing, smart — what is safe, manual — everything.",
                    ),
                };
                (
                    title.to_string(),
                    v_flex()
                        .w_full()
                        .py_1()
                        .children(items)
                        .child(div().px_3().pt_2().text_sm().text_color(muted).child(note))
                        .into_any_element(),
                )
            }
            _ => return None,
        };
        let close =
            super::inserts::icon_button("env-card-close", crate::icons::Ico::X, "Close", &theme)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_card(kind, name.clone(), window, cx)
                }));
        let head = super::block_head(&theme, true)
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.foreground)
                    .child(title),
            )
            .children(actions)
            .child(close);
        let card = super::inserts::card(head, body, &theme);
        if follow.is_empty() {
            return Some(card);
        }
        Some(
            v_flex()
                .w_full()
                .gap_4()
                .child(card)
                .children(follow)
                .into_any_element(),
        )
    }

    /// The open skill's tree (folders open in place, a file opens under it; a row's rename and
    /// trash on hover), the field naming a new file or folder, and the open file's card.
    fn skill_parts(
        &self,
        k: &crate::app::project_env::SkillInfo,
        cx: &mut Context<Self>,
    ) -> (AnyElement, Option<AnyElement>, Option<AnyElement>) {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return (div().into_any_element(), None, None);
        };
        let shown = |rel: &str| {
            let mut up = rel;
            while let Some((p, _)) = up.rsplit_once('/') {
                if !e.skill_expanded.contains(p) {
                    return false;
                }
                up = p;
            }
            true
        };
        let mut rows: Vec<AnyElement> = vec![
            div()
                .px_3()
                .py_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(muted)
                .child(k.dir.clone())
                .into_any_element(),
        ];
        for (n, (rel, dir)) in e.skill_tree.iter().filter(|(r, _)| shown(r)).enumerate() {
            let (rel, dir) = (rel.clone(), *dir);
            let depth = rel.matches('/').count();
            let name = rel.rsplit('/').next().unwrap_or_default().to_string();
            let open = e.skill_expanded.contains(&rel);
            let chosen = !dir && e.skill_file.as_deref() == Some(rel.as_str());
            let group = SharedString::from(format!("env-skill-g{n}"));
            let armed = e.armed.as_deref() == Some(format!("skillfile:{rel}").as_str());
            let (ico, color) = super::file_icon(&name, dir, &theme);
            let chevron = if open {
                crate::icons::Ico::ChevronDown
            } else {
                crate::icons::Ico::ChevronRight
            };
            let (r1, r2) = (rel.clone(), rel.clone());
            let rename = super::inserts::icon_button(
                &format!("env-skill-mv{n}"),
                crate::icons::Ico::Pencil,
                "Rename",
                &theme,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.click_claimed = true;
                cx.stop_propagation();
                this.skill_form("rename", r1.clone(), window, cx)
            }));
            let trash = div()
                .id(SharedString::from(format!("env-skill-rm{n}")))
                .flex_none()
                .px_1()
                .cursor_pointer()
                .text_sm()
                .when(!armed, |d| {
                    d.text_color(muted)
                        .hover(|s| s.text_color(theme.danger))
                        .child(crate::icons::Ico::Trash.icon().small())
                })
                .when(armed, |d| d.text_color(theme.danger).child("Delete?"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    this.skill_delete(r2.clone(), window, cx)
                }));
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("env-skill-row{n}")))
                    .group(group.clone())
                    .pl(px(12. + 16. * depth as f32))
                    .pr_3()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .border_t_1()
                    .border_color(theme.border.opacity(0.5))
                    .when(chosen, |d| d.bg(theme.list_active))
                    .hover(|s| s.bg(theme.list_hover))
                    .child(
                        div()
                            .w(px(12.))
                            .flex_none()
                            .text_color(muted)
                            .when(dir, |d| d.child(chevron.icon().xsmall())),
                    )
                    .child(ico.icon().small().text_color(color))
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .child(
                        h_flex()
                            .flex_none()
                            .when(!armed, |d| {
                                d.opacity(0.).group_hover(group, |s| s.opacity(1.))
                            })
                            .child(rename)
                            .child(trash),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.skill_entry(rel.clone(), dir, window, cx)
                    }))
                    .into_any_element(),
            );
        }
        if e.skill_tree.is_empty() {
            rows.push(
                div()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(theme.border.opacity(0.5))
                    .text_color(muted)
                    .child("Reading the folder…")
                    .into_any_element(),
            );
        }
        let tree = v_flex().w_full().children(rows).into_any_element();

        let form = e.skill_form.as_ref().map(|(what, target)| {
            let title = match *what {
                "file" => "New file",
                "folder" => "New folder",
                _ => "Rename",
            };
            let place = if *what == "rename" {
                target.clone()
            } else if target.is_empty() {
                format!("In {}", k.name)
            } else {
                format!("In {}/{target}", k.name)
            };
            let actions = vec![
                super::form_action(
                    "env-skill-form-ok",
                    if *what == "rename" {
                        "Rename"
                    } else {
                        "Create"
                    },
                    true,
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.click_claimed = true;
                    this.skill_form_done(window, cx)
                }))
                .into_any_element(),
                super::form_action("env-skill-form-cancel", "Cancel", false, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.click_claimed = true;
                        if let Some(e) = this.project_env.as_mut() {
                            e.skill_form = None;
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            ];
            super::inline_form(
                title,
                vec![(
                    "Name",
                    gpui_kit::component::input::Input::new(&e.skill_form_name).into_any_element(),
                )],
                Some(place.into()),
                actions,
                cx,
            )
        });

        let file = e.skill_file.clone().map(|rel| {
            let dirty = e.skill_dirty;
            let save = div()
                .id("env-skill-file-save")
                .flex_none()
                .px_2()
                .rounded_md()
                .when(dirty, |d| {
                    d.cursor_pointer()
                        .bg(theme.primary)
                        .text_color(theme.primary_foreground)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.click_claimed = true;
                            this.env_save("skill", window, cx)
                        }))
                })
                .when(!dirty, |d| d.text_color(muted))
                .child("Save");
            let revert = dirty.then(|| {
                div()
                    .id("env-skill-file-revert")
                    .flex_none()
                    .cursor_pointer()
                    .text_color(muted)
                    .hover(|s| s.opacity(0.8))
                    .child("Revert")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_revert("skill", window, cx)
                    }))
            });
            let close = super::inserts::icon_button(
                "env-skill-file-close",
                crate::icons::Ico::X,
                "Close the file",
                &theme,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.click_claimed = true;
                if let Some(e) = this.project_env.as_mut().filter(|e| !e.skill_dirty) {
                    e.skill_file = None;
                }
                cx.notify();
            }));
            let head = super::block_head(&theme, true)
                .gap_3()
                .child(crate::icons::Ico::File.icon().small())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(theme.foreground)
                        .child(rel),
                )
                .child(save)
                .children(revert)
                .child(close);
            super::inserts::card(head, code(&e.skill_text, cx), &theme)
        });
        (tree, form, file)
    }
}

/// A text: a code editor as tall as its wrapped text (the thread scrolls, not it).
fn code(state: &Entity<gpui_kit::component::input::EditorState>, cx: &App) -> AnyElement {
    let text = state.read(cx).value().to_string();
    let lines: usize = text
        .lines()
        .map(|l| l.chars().count() / 100 + 1)
        .sum::<usize>()
        .max(1);
    div()
        .h(px(super::editor_height(lines, cx)))
        .child(
            gpui_kit::component::input::Editor::new(state)
                .appearance(false)
                .h_full(),
        )
        .into_any_element()
}

/// A run's time as read: "today 18:00", "tomorrow 09:00", "Mon 2 Nov 09:00".
fn when(iso: &str) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) else {
        return iso.to_string();
    };
    let t = t.with_timezone(&chrono::Local);
    let today = chrono::Local::now().date_naive();
    match (t.date_naive() - today).num_days() {
        0 => t.format("today %H:%M").to_string(),
        1 => t.format("tomorrow %H:%M").to_string(),
        -1 => t.format("yesterday %H:%M").to_string(),
        _ => t.format("%a %-d %b %H:%M").to_string(),
    }
}
