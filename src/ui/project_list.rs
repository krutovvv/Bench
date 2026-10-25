//! The new project's form (the sidebar's "+"): a name and, if it has one, a folder on disk.

use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
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
    /// A project's environment as rows in blocks (the task list's look: a line between rows, a
    /// row lit under the pointer); a row's click opens its card under the block, as the agent's
    /// answer (`render_env_card`), where it is edited, saved or removed.
    pub(super) fn render_env_sections(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<(&'static str, AnyElement)> {
        let _ = window;
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let Some(e) = self.project_env.as_ref() else {
            return vec![("", div().text_color(muted).child("Loading…").into_any_element())];
        };
        let remote = self.projects.iter().any(|p| p.id == e.id && p.host.is_some());
        let general = e.id == "general";
        let card = e.card.clone();
        // A quiet text action in a head: muted, the foreground on hover.
        let act = |id: SharedString, label: &'static str| {
            div().id(id).flex_none().cursor_pointer().text_color(muted).hover(|s| s.text_color(theme.foreground)).child(label)
        };
        // "+ Add": a new one's card (paste it, or ask the agent from there); accesses are the
        // agent's to add.
        let add = |id: &'static str, kind: &'static str, ask: &'static str, cx: &mut Context<Self>| {
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
        // A row: its name, a one-line note; lit when its card is open.
        let row = |kind: &'static str, name: String, label: String, note: String, bad: bool, first: bool, cx: &mut Context<Self>| {
            let note = note.split_whitespace().collect::<Vec<_>>().join(" ");
            let on = card.as_ref().is_some_and(|(k, n)| *k == kind && *n == name);
            h_flex()
                .id(SharedString::from(format!("env-{kind}-{name}")))
                .px_3()
                .py_1p5()
                .gap_2()
                .cursor_pointer()
                .when(!first, |d| d.border_t_1().border_color(theme.border.opacity(0.5)))
                .when(on, |d| d.bg(theme.list_active))
                .hover(|s| s.bg(theme.list_hover))
                .child(div().flex_none().child(label))
                .child(div().flex_1().min_w_0().truncate().text_color(if bad { theme.danger } else { muted }).child(note))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_card(kind, name.clone(), window, cx)
                }))
                .into_any_element()
        };
        let empty = |text: &'static str| div().px_3().py_2().text_color(muted).child(text).into_any_element();
        let folded = e.folded.clone();
        let section = |title: &'static str, extra: Option<AnyElement>, rows: Vec<AnyElement>, cx: &mut Context<Self>| {
            let shut = folded.contains(title);
            let head = super::block_head(&theme, !shut)
                .id(SharedString::from(format!("env-sec-{title}")))
                .cursor_pointer()
                .child(if shut { crate::icons::Ico::ChevronRight } else { crate::icons::Ico::ChevronDown }.icon().xsmall())
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
            rows.push(row("name", String::new(), "Name".into(), e.name.read(cx).value().to_string(), false, true, cx));
        }
        let soul_text = e.soul.read(cx).value().to_string();
        let first = soul_text.lines().map(|l| l.trim_start_matches('#').trim()).find(|l| !l.is_empty()).unwrap_or("not described yet").to_string();
        rows.push(row("soul", String::new(), "Soul".into(), first, false, general, cx));
        let tg_note = match &e.telegram {
            Some((name, users, true)) => format!("{} · {} may write", if name.is_empty() { "a bot".to_string() } else { format!("@{name}") }, users.len()),
            _ => "no bot of its own".into(),
        };
        rows.push(row("telegram", String::new(), "Telegram".into(), tg_note, false, false, cx));
        blocks.push((if general { "Host" } else { "Project" }, section(if general { "Host" } else { "Project" }, None, rows, cx)));

        // MCP servers.
        let mut rows = vec![];
        if e.mcp.is_empty() {
            rows.push(empty("None of its own: the general ones work here too."));
        }
        for (k, (name, kind, enabled, connected, tools, error, _)) in e.mcp.clone().into_iter().enumerate() {
            let note = match (enabled, &error, connected) {
                (false, _, _) => format!("{kind} · off"),
                (_, Some(err), _) => format!("{kind} · {err}"),
                (_, _, true) => format!("{kind} · {tools} tools"),
                _ => format!("{kind} · connects with the project's first thread"),
            };
            rows.push(row("mcp", name.clone(), name, note, error.is_some() && enabled, k == 0, cx));
        }
        let extra = add("env-mcp-add", "mcp", "", cx).into_any_element();
        blocks.push(("MCP servers", section("MCP servers", Some(extra), rows, cx)));

        // Skills.
        let mut rows = vec![];
        if e.skills.is_empty() {
            rows.push(empty("None of its own: the general ones work here too."));
        }
        for (k, (name, desc)) in e.skills.clone().into_iter().enumerate() {
            rows.push(row("skill", name.clone(), name, desc, false, k == 0, cx));
        }
        let extra = add("env-skill-add", "skill", "", cx).into_any_element();
        blocks.push(("Skills", section("Skills", Some(extra), rows, cx)));

        // Prompts (@ in the composer).
        let mut rows = vec![];
        if e.prompts.is_empty() {
            rows.push(empty("None of its own: @ in the composer offers the general ones."));
        }
        for (k, name) in e.prompts.clone().into_iter().enumerate() {
            rows.push(row("prompt", name.clone(), name, String::new(), false, k == 0, cx));
        }
        let extra = add("env-prompt-add", "prompt", "", cx).into_any_element();
        blocks.push(("Prompts", section("Prompts", Some(extra), rows, cx)));

        // Schedule: what runs on its own, each run a thread of the project.
        let mut rows = vec![];
        if e.cron.is_empty() {
            rows.push(empty("Nothing runs on its own yet: a job runs its prompt in a new thread of the project, on a schedule."));
        }
        for (k, j) in e.cron.clone().into_iter().enumerate() {
            let next = j.next_run.as_deref().map(when).unwrap_or_else(|| "no next run".into());
            let mut note = if j.enabled { format!("{} · next {next}", j.display) } else { format!("{} · paused", j.display) };
            if let Some(st) = &j.last_status {
                note.push_str(&format!(" · last: {st}"));
            }
            let failed = j.last_status.as_deref().is_some_and(|s| s != "ok");
            rows.push(row("cron", j.id.clone(), j.name.clone(), note, failed, k == 0, cx));
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
                rows.push(empty("No servers or file hosts yet: /ssh and /ftp in its threads offer only these."));
            }
            for (k, (hid, name, kind, at)) in e.hosts.clone().into_iter().enumerate() {
                rows.push(row("host", hid, name, format!("{kind} · {at}"), false, k == 0, cx));
            }
            let extra = add("env-host-add", "", "Add to this project the access: ", cx).into_any_element();
            blocks.push(("Accesses", section("Accesses", Some(extra), rows, cx)));
        }

        if let Some(err) = e.error.clone() {
            blocks.push(("", div().text_color(theme.danger).child(err).into_any_element()));
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
            div().id(SharedString::from(id)).flex_none().cursor_pointer().text_color(color).hover(|s| s.opacity(0.8)).child(label)
        };
        let save = |what: &'static str, dirty: bool, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("env-card-save-{what}")))
                .flex_none()
                .px_2()
                .rounded_md()
                .when(dirty, |d| {
                    d.cursor_pointer().bg(theme.primary).text_color(theme.primary_foreground).on_click(cx.listener(move |this, _, window, cx| {
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
        let remove = |what: &'static str, target: String, label: &'static str, cx: &mut Context<Self>| {
            let on = armed.as_deref() == Some(format!("{what}:{target}").as_str());
            act(format!("env-card-rm-{what}"), if on { format!("{label}?") } else { label.to_string() }, theme.danger)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.click_claimed = true;
                    this.env_remove(what, target.clone(), window, cx)
                }))
                .into_any_element()
        };
        // A text: a code editor as tall as its wrapped text (the thread scrolls, not it).
        let code = |state: &Entity<gpui_kit::component::input::EditorState>, cx: &App| {
            let text = state.read(cx).value().to_string();
            let lines: usize = text.lines().map(|l| l.chars().count() / 100 + 1).sum::<usize>().max(1);
            div()
                .h(px(super::editor_height(lines, cx)))
                .child(gpui_kit::component::input::Editor::new(state).appearance(false).h_full())
                .into_any_element()
        };
        let pad = |d: Div| d.px_3().py_2().gap_2().into_any_element();
        let mut actions: Vec<AnyElement> = vec![];
        let (title, body): (String, AnyElement) = match kind {
            "name" => {
                actions.push(save("name", e.name_dirty, cx));
                actions.extend(revert("name", e.name_dirty, cx));
                actions.push(remove("project", String::new(), "Remove the project", cx));
                ("Name".into(), pad(v_flex().child(gpui_kit::component::input::Input::new(&e.name)).child(div().text_sm().text_color(muted).child("Removing the project keeps its threads."))))
            }
            "soul" => {
                actions.push(save("soul", e.soul_dirty, cx));
                actions.extend(revert("soul", e.soul_dirty, cx));
                ("Soul: what the project is, how to work in it".into(), code(&e.soul, cx))
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
                    "mcp" => ("New MCP server: its name, and its config (Pi's format: {\"url\", \"headers\"} or {\"command\", \"args\", \"env\"})", &e.mcp_json, true),
                    "skill" => ("New skill: paste its SKILL.md (its front matter names it)", &e.skill_text, false),
                    _ => ("New prompt: its name, and its text", &e.prompt_text, true),
                };
                let body = v_flex()
                    .when(named, |d| d.child(div().px_3().pt_2().child(gpui_kit::component::input::Input::new(&e.new_name))))
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
            "skill" => {
                if !e.skills.iter().any(|k| k.0 == name) {
                    return None;
                }
                actions.push(save("skill", e.skill_dirty, cx));
                actions.extend(revert("skill", e.skill_dirty, cx));
                actions.push(remove("skill", name.clone(), "Remove", cx));
                (format!("Skill · {name}"), code(&e.skill_text, cx))
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
                        act("env-card-pause".into(), if enabled { "Pause".into() } else { "Resume".into() }, muted)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.click_claimed = true;
                                this.env_cron(pause.clone(), if enabled { "pause" } else { "resume" }, window, cx)
                            }))
                            .into_any_element(),
                    );
                }
                actions.push(save("cron", e.cron_dirty, cx));
                actions.extend(revert("cron", e.cron_dirty, cx));
                if job.is_some() {
                    actions.push(remove("cron", name.clone(), "Remove", cx));
                }
                let last = job.as_ref().filter(|j| !j.last_output.trim().is_empty()).map(|j| {
                    let at = j.last_run.as_deref().map(when).unwrap_or_default();
                    div().text_sm().text_color(muted).child(format!("Last run {at}: {}", j.last_output.trim()))
                });
                (
                    job.as_ref().map(|j| format!("Job · {}", j.name)).unwrap_or_else(|| "New job".into()),
                    pad(v_flex()
                        .child(gpui_kit::component::input::Input::new(&e.cron_name))
                        .child(gpui_kit::component::input::Input::new(&e.cron_schedule))
                        .child(div().border_1().rounded_md().border_color(theme.border).child(code(&e.cron_prompt, cx)))
                        .children(last)),
                )
            }
            "host" => {
                let (hid, hname, hkind, at) = e.hosts.iter().find(|h| h.0 == name).cloned()?;
                actions.push(remove("host", hid, "Remove from the project", cx));
                (format!("Access · {hname}"), pad(v_flex().child(div().text_color(muted).child(format!("{hkind} · {at}")))))
            }
            _ => return None,
        };
        let close = super::inserts::icon_button("env-card-close", crate::icons::Ico::X, "Close", &theme).on_click(cx.listener(move |this, _, window, cx| {
            this.click_claimed = true;
            this.env_card(kind, name.clone(), window, cx)
        }));
        let head = super::block_head(&theme, true)
            .gap_3()
            .child(div().flex_1().min_w_0().truncate().text_color(theme.foreground).child(title))
            .children(actions)
            .child(close);
        Some(super::inserts::card(head, body, &theme))
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
