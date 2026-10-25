//! The new project's form (the sidebar's "+"): a name and, if it has one, a folder on disk.

use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::BenchApp;

impl BenchApp {
    /// A new project, inside the thread: its name, where its agent runs (this Mac, a server from
    /// ~/.ssh/config, or another one typed in), a folder on disk if it has one.
    pub(super) fn render_project_form(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(form) = self.project_form.as_ref() else {
            return div().into_any_element();
        };
        let host = form.host.clone();
        let folder = form.folder.clone();
        let mut places: Vec<(Option<String>, String)> = vec![(None, "This Mac".into())];
        places.extend(self.ssh.servers.iter().map(|s| (Some(s.alias.clone()), s.alias.clone())));
        places.push((Some(String::new()), "Another server…".into()));
        let where_row = h_flex()
            .flex_wrap()
            .gap_1()
            .children(places.into_iter().enumerate().map(|(k, (h, label))| {
                let on = h == host;
                div()
                    .id(("project-where", k))
                    .px_2()
                    .cursor_pointer()
                    .when(on, |d| d.bg(theme.primary).text_color(theme.primary_foreground))
                    .when(!on, |d| d.text_color(theme.muted_foreground).hover(|s| s.text_color(theme.foreground)))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.click_claimed = true;
                        if let Some(f) = this.project_form.as_mut() {
                            f.host = h.clone();
                        }
                        cx.notify();
                    }))
            }))
            .into_any_element();
        let mut rows = vec![("Name", Input::new(&form.name).into_any_element()), ("Where", where_row)];
        if host.as_deref() == Some("") {
            rows.push(("Address", Input::new(&form.new_host).into_any_element()));
            rows.push(("User", Input::new(&form.new_user).into_any_element()));
            rows.push(("Port", Input::new(&form.new_port).into_any_element()));
            rows.push(("Key file", Input::new(&form.new_key).into_any_element()));
        }
        if host.is_none() {
            rows.push((
                "Folder",
                h_flex()
                    .gap_3()
                    .child(div().text_color(if folder.is_some() { theme.foreground } else { theme.muted_foreground }).child(folder.clone().unwrap_or_else(|| "none: it lives in Bench".into())))
                    .child(super::form_action("project-folder", "Choose…", false, cx).on_click(cx.listener(|this, _, window, cx| {
                        this.click_claimed = true;
                        this.pick_project_codebase(window, cx)
                    })))
                    .into_any_element(),
            ));
        }
        let note: SharedString = if host.is_some() {
            "Bench installs and starts the agent there over SSH (Node.js 22.18+ on it) and keeps the threads, skills and MCP there; on its first start it takes over what Hermes kept on that server.".into()
        } else {
            "Its threads work in its folder; servers, file hosts, the browser and tasks are attached in a thread with /.".into()
        };
        let actions = vec![
            super::form_action("project-save", "Create", true, cx)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.click_claimed = true;
                    this.save_project(window, cx)
                }))
                .into_any_element(),
            super::form_action("project-cancel", "Cancel", false, cx)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.project_form = None;
                    cx.notify();
                }))
                .into_any_element(),
        ];
        super::inline_form("New project", rows, Some(note), actions, cx)
    }
}

impl BenchApp {
    /// A project's environment at the top of its thread, as a document of rows: its name, soul,
    /// Telegram bot, MCP servers, skills, prompts and accesses. A row opens in place as a code
    /// block (line numbers, highlighting) with Save / Revert under it, and closes again; "+ Add"
    /// starts the request for the agent in the composer.
    pub(super) fn render_env_block(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _ = window;
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return div().text_color(muted).child("Loading…").into_any_element();
        };
        let remote = self.projects.iter().any(|p| p.id == e.id && p.host.is_some());
        // The general environment has no name to change and is not removed.
        let general = e.id == "general";
        // A quiet text action: muted, the foreground on hover; red for removing.
        let act = |id: SharedString, label: String, danger: bool| {
            div()
                .id(id)
                .flex_none()
                .cursor_pointer()
                .text_color(if danger { theme.danger } else { muted })
                .hover(|s| s.text_color(if danger { theme.danger } else { theme.foreground }))
                .child(label)
        };
        // Under an opened block: Save (live once something changed) and Revert.
        let footer = |id: &'static str, what: &'static str, dirty: bool, cx: &mut Context<Self>| {
            h_flex()
                .pt_1()
                .pb_2()
                .gap_3()
                .child(
                    div()
                        .id(SharedString::from(format!("{id}-save")))
                        .px_2()
                        .when(dirty, |d| {
                            d.cursor_pointer()
                                .bg(theme.primary)
                                .text_color(theme.primary_foreground)
                                .hover(|s| s.bg(theme.primary_hover))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.click_claimed = true;
                                    this.env_save(what, window, cx)
                                }))
                        })
                        .when(!dirty, |d| d.border_1().rounded_md().border_color(theme.border).text_color(muted))
                        .child("Save"),
                )
                .when(dirty, |d| {
                    d.child(act(format!("{id}-revert").into(), "Revert".into(), false).on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_revert(what, window, cx)
                    })))
                })
                .into_any_element()
        };
        // An opened text: a code block, as tall as its wrapped text (the thread scrolls, not it).
        let code = |state: &Entity<gpui_kit::component::input::EditorState>, cx: &App| {
            let text = state.read(cx).value().to_string();
            let lines: usize = text.lines().map(|l| l.chars().count() / 100 + 1).sum::<usize>().max(1);
            div()
                .mt_1()
                .border_1().rounded_md()
                .border_color(theme.border)
                .h(px(lines as f32 * 21. + 10.))
                .child(gpui_kit::component::input::Editor::new(state).appearance(false).h_full())
                .into_any_element()
        };
        let heading = |title: &'static str| div().font_weight(FontWeight::MEDIUM).child(title);
        let armed = e.armed.clone();
        let is_armed = |k: String| armed.as_deref() == Some(&k);
        let remove = |id: String, what: &'static str, name: String, cx: &mut Context<Self>| {
            let on = is_armed(format!("{what}:{name}"));
            div()
                .flex_none()
                .when(!on, |d| d.opacity(0.).group_hover(SharedString::from(id.clone()), |s| s.opacity(1.)))
                .child(act(format!("{id}-rm").into(), if on { "Remove?".into() } else { "Remove".into() }, true).on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    cx.stop_propagation();
                    this.env_remove(what, name.clone(), window, cx)
                })))
        };
        let add = |id: &'static str, ask: &'static str, cx: &mut Context<Self>| {
            act(id.into(), "+ Add".into(), false).on_click(cx.listener(move |this, _, window, cx| {
                this.click_claimed = true;
                this.ask_in_composer(ask, window, cx)
            }))
        };
        let row = |id: String, name: String, note: String, bad: bool| {
            h_flex()
                .id(SharedString::from(id.clone()))
                .group(SharedString::from(id))
                .gap_2()
                .py_0p5()
                .child(div().flex_none().child(name))
                .child(div().flex_1().min_w_0().truncate().text_color(if bad { theme.danger } else { muted }).child(note))
        };
        let toggle = |which: &'static str| {
            move |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                this.click_claimed = true;
                if let Some(e) = this.project_env.as_mut() {
                    match which {
                        "soul" => e.soul_open = !e.soul_open,
                        _ => e.tg_open = !e.tg_open,
                    }
                }
                cx.notify();
            }
        };
        let mut out = v_flex().w_full();

        // Its name (a remote project's is its server's agent: named here, on this Mac, too).
        out = out.when(!general, |d| d.child(
            h_flex()
                .gap_3()
                .child(div().flex_none().text_color(muted).child("Name"))
                .child(div().w(px(320.)).child(gpui_kit::component::input::Input::new(&e.name).appearance(false)))
                .when(e.name_dirty, |d| d.child(footer("name", "name", true, cx))),
        ));

        // The soul.
        let soul_text = e.soul.read(cx).value().to_string();
        let first = soul_text.lines().map(|l| l.trim_start_matches('#').trim()).find(|l| !l.is_empty()).unwrap_or("not described yet").to_string();
        out = out.child(
            row("env-soul".into(), "Soul".into(), if e.soul_open { "what the project is, how to work in it".into() } else { first }, false)
                .cursor_pointer()
                .on_click(cx.listener(toggle("soul"))),
        );
        if e.soul_open {
            out = out.child(code(&e.soul, cx)).child(footer("soul", "soul", e.soul_dirty, cx));
        }

        // Its Telegram bot.
        let tg_note = match &e.telegram {
            Some((name, users, true)) => format!("{} · {} may write", if name.is_empty() { "a bot".to_string() } else { format!("@{name}") }, users.len()),
            _ => "no bot of its own: open to set one".into(),
        };
        out = out.child(row("env-tg".into(), "Telegram".into(), tg_note, false).cursor_pointer().on_click(cx.listener(toggle("telegram"))));
        if e.tg_open {
            out = out
                .child(
                    v_flex()
                        .mt_1()
                        .gap_2()
                        .child(gpui_kit::component::input::Input::new(&e.tg_token))
                        .child(gpui_kit::component::input::Input::new(&e.tg_users))
                        .child(div().text_color(muted).child("Its chats become this project's threads, with its soul, skills and servers.")),
                )
                .child(footer("tg", "telegram", e.tg_dirty, cx));
        }

        // MCP servers: each opens to its own config.
        out = out.child(h_flex().pt_4().gap_3().child(heading("MCP servers")).child(add("env-mcp-add", "Add an MCP server to this project: ", cx)));
        if e.mcp.is_empty() {
            out = out.child(div().text_color(muted).child("None of its own: the general ones work here too."));
        }
        for (name, kind, enabled, connected, tools, error, _) in e.mcp.clone() {
            let note = match (enabled, &error, connected) {
                (false, _, _) => format!("{kind} · off (\"enabled\": false)"),
                (_, Some(err), _) => format!("{kind} · {err}"),
                (_, _, true) => format!("{kind} · {tools} tools"),
                _ => format!("{kind} · connects with the project's first thread"),
            };
            let id = format!("env-mcp-{name}");
            let n = name.clone();
            out = out.child(
                row(id.clone(), name.clone(), note, error.is_some() && enabled)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_open("mcp", n.clone(), window, cx)
                    }))
                    .child(remove(id, "mcp", name.clone(), cx)),
            );
            if e.mcp_open.as_deref() == Some(&name) {
                out = out.child(code(&e.mcp_json, cx)).child(footer("mcp", "mcp", e.mcp_dirty, cx));
            }
        }

        // Skills.
        out = out.child(h_flex().pt_4().gap_3().child(heading("Skills")).child(add("env-skill-add", "Write a skill for this project: ", cx)));
        if e.skills.is_empty() {
            out = out.child(div().text_color(muted).child("None of its own: the general ones work here too."));
        }
        for (name, desc) in e.skills.clone() {
            let id = format!("env-skill-{name}");
            let n = name.clone();
            out = out.child(
                row(id.clone(), name.clone(), desc, false)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_open_skill(n.clone(), window, cx)
                    }))
                    .child(remove(id, "skill", name.clone(), cx)),
            );
            if e.skill_open.as_deref() == Some(&name) {
                out = out.child(code(&e.skill_text, cx)).child(footer("skill", "skill", e.skill_dirty, cx));
            }
        }

        // Prompts (@ in the composer).
        out = out.child(h_flex().pt_4().gap_3().child(heading("Prompts")).child(add("env-prompt-add", "Write a prompt for this project: ", cx)));
        if e.prompts.is_empty() {
            out = out.child(div().text_color(muted).child("None of its own: @ in the composer offers the general ones."));
        }
        for name in e.prompts.clone() {
            let id = format!("env-prompt-{name}");
            let n = name.clone();
            out = out.child(
                row(id.clone(), name.clone(), String::new(), false)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_open("prompt", n.clone(), window, cx)
                    }))
                    .child(remove(id, "prompt", name.clone(), cx)),
            );
            if e.prompt_open.as_deref() == Some(&name) {
                out = out.child(code(&e.prompt_text, cx)).child(footer("prompt", "prompt", e.prompt_dirty, cx));
            }
        }

        // Schedule: what runs on its own, each run a thread of the project.
        out = out.child(
            h_flex().pt_4().gap_3().child(heading("Schedule")).child(act("env-cron-add".into(), "+ New job".into(), false).on_click(cx.listener(|this, _, window, cx| {
                this.click_claimed = true;
                this.env_open_cron("new".into(), window, cx)
            }))),
        );
        if e.cron.is_empty() && e.cron_open.as_deref() != Some("new") {
            out = out.child(div().text_color(muted).child("Nothing runs on its own yet: a job runs its prompt in a new thread of the project, on a schedule."));
        }
        let cron_form = |e: &crate::app::ProjectEnv, cx: &mut Context<Self>| {
            v_flex()
                .mt_1()
                .gap_2()
                .child(gpui_kit::component::input::Input::new(&e.cron_name))
                .child(gpui_kit::component::input::Input::new(&e.cron_schedule))
                .child(code(&e.cron_prompt, cx))
                .child(footer("cron", "cron", e.cron_dirty, cx))
                .into_any_element()
        };
        if e.cron_open.as_deref() == Some("new") {
            out = out.child(cron_form(e, cx));
        }
        for j in e.cron.clone() {
            let id = format!("env-cron-{}", j.id);
            let next = j.next_run.as_deref().map(when).unwrap_or_else(|| "no next run".into());
            let mut note = if j.enabled { format!("{} · next {next}", j.display) } else { format!("{} · paused", j.display) };
            if let Some(st) = &j.last_status {
                note.push_str(&format!(" · last: {st}"));
            }
            let failed = j.last_status.as_deref().is_some_and(|s| s != "ok");
            let (open_id, pause_id, run_id) = (j.id.clone(), j.id.clone(), j.id.clone());
            let enabled = j.enabled;
            let hover = SharedString::from(id.clone());
            out = out.child(
                row(id.clone(), j.name.clone(), note, failed)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.click_claimed = true;
                        this.env_open_cron(open_id.clone(), window, cx)
                    }))
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_3()
                            .opacity(0.)
                            .group_hover(hover, |s| s.opacity(1.))
                            .child(act(format!("{id}-run").into(), "Run now".into(), false).on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                cx.stop_propagation();
                                this.env_cron(run_id.clone(), "trigger", window, cx)
                            })))
                            .child(act(format!("{id}-pause").into(), if enabled { "Pause".into() } else { "Resume".into() }, false).on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                cx.stop_propagation();
                                this.env_cron(pause_id.clone(), if enabled { "pause" } else { "resume" }, window, cx)
                            }))),
                    )
                    .child(remove(id, "cron", j.id.clone(), cx)),
            );
            if e.cron_open.as_deref() == Some(j.id.as_str()) {
                out = out.child(cron_form(e, cx));
                if !j.last_output.trim().is_empty() {
                    let last = j.last_run.as_deref().map(when).unwrap_or_default();
                    out = out.child(div().pb_2().text_sm().text_color(muted).child(format!("Last run {last}: {}", j.last_output.trim())));
                }
            }
        }

        // Accesses.
        if !remote {
            out = out.child(h_flex().pt_4().gap_3().child(heading("Accesses")).child(add("env-host-add", "Add to this project the access: ", cx)));
            if e.hosts.is_empty() {
                out = out.child(div().text_color(muted).child("No servers or file hosts yet: /ssh and /ftp in its threads offer only these."));
            }
            for (hid, name, kind, at) in e.hosts.clone() {
                let id = format!("env-host-{hid}");
                out = out.child(row(id.clone(), name, format!("{kind} · {at}"), false).child(remove(id, "host", hid, cx)));
            }
        }

        // The project's removal, in red.
        let project_armed = is_armed("project:".into());
        out = out
            .when_some(e.error.clone(), |d, err| d.child(div().pt_2().text_color(theme.danger).child(err)))
            .when(!general, |d| d.child(
                h_flex().pt_4().child(div().flex_1()).child(
                    act("env-remove-project".into(), if project_armed { "Remove the project? Its threads stay".into() } else { "Remove project".into() }, true)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.click_claimed = true;
                            this.env_remove("project", String::new(), window, cx)
                        })),
                ),
            ));
        out.into_any_element()
    }
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
