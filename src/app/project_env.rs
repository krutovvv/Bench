//! A project's environment as its thread's block (`env:<project id>`): its soul, MCP servers,
//! skills and accesses, edited in place (a Save appears once something changed) or through the
//! agent in the conversation under it.

use gpui_kit::component::input::{EditorState, InputEvent, InputState};
use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::{Value, json};

use super::BenchApp;
use crate::rest;

pub struct ProjectEnv {
    pub id: String,
    /// Where it lives (`~/.bench/projects/<id>`).
    pub dir: String,
    /// Its soul (SYSTEM.md): a row, opened to edit.
    pub soul: Entity<EditorState>,
    pub soul_open: bool,
    pub soul_dirty: bool,
    /// Its own Telegram bot: (@name, allowed user ids, its token); a row, opened to edit.
    pub telegram: Option<(String, Vec<String>, String)>,
    pub tg_open: bool,
    pub tg_token: Entity<InputState>,
    pub tg_users: Entity<InputState>,
    pub tg_dirty: bool,
    /// The project's name, editable at the block's top.
    pub name: Entity<InputState>,
    pub name_dirty: bool,
    /// MCP servers: what each is, and what the agent knows of it (`McpInfo`).
    pub mcp: Vec<McpInfo>,
    /// A check of them is in flight (the agent is shaking hands with each).
    pub mcp_checking: bool,
    /// MCP servers that asked for OAuth: "Authorize" signs in.
    pub mcp_auth: std::collections::HashSet<String>,
    /// The server opened under its row, and its config being edited.
    pub mcp_open: Option<String>,
    pub mcp_json: Entity<EditorState>,
    pub mcp_dirty: bool,
    /// Its own prompts (@ in the composer), and the one opened under its row.
    pub prompts: Vec<String>,
    pub prompt_open: Option<String>,
    pub prompt_text: Entity<EditorState>,
    pub prompt_dirty: bool,
    /// Every skill its threads can use, with where each lives.
    pub skills: Vec<SkillInfo>,
    /// The skill opened under its row (its folder as a tree), and the file of it being edited.
    pub skill_open: Option<String>,
    pub skill_text: Entity<EditorState>,
    pub skill_dirty: bool,
    /// The open skill's folder: (path inside it, is a folder), folders first at each level.
    pub skill_tree: Vec<(String, bool)>,
    /// Its folders shown open, and the one a new file or folder goes into ("" its top).
    pub skill_expanded: std::collections::HashSet<String>,
    pub skill_folder: String,
    /// The file of it open under the tree (a path inside the skill).
    pub skill_file: Option<String>,
    /// A new file or folder, or a rename, being named: ("file" | "folder" | "rename", the
    /// folder it goes into or the entry renamed), and the name.
    pub skill_form: Option<(&'static str, String)>,
    pub skill_form_name: Entity<InputState>,
    /// Its accesses: SSH (a mirror of ~/.ssh/config) and file hosts (its own).
    pub hosts: Vec<Access>,
    /// The access opened under its row (its id), and the fields being edited. The password is
    /// never read back: an empty field keeps the one that is saved.
    pub host_open: Option<String>,
    pub h_name: Entity<InputState>,
    pub h_host: Entity<InputState>,
    pub h_port: Entity<InputState>,
    pub h_user: Entity<InputState>,
    pub h_password: Entity<InputState>,
    pub h_path: Entity<InputState>,
    pub host_dirty: bool,
    /// Its scheduled jobs; the one opened under its row ("new": a new one's form) and its fields.
    pub cron: Vec<CronJob>,
    pub cron_open: Option<String>,
    pub cron_name: Entity<InputState>,
    pub cron_schedule: Entity<InputState>,
    pub cron_prompt: Entity<EditorState>,
    pub cron_dirty: bool,
    /// A new MCP server's or prompt's name (a new skill names itself in its front matter).
    pub new_name: Entity<InputState>,
    /// How the project's threads run: how hard it thinks, what it may run without asking. Chosen
    /// here once, not per chat; the model is one for every project (the gear).
    pub effort: String,
    pub approvals: String,
    /// What is open under the block, as the agent's answer: (kind, name) — "name", "soul",
    /// "telegram", "mcp", "skill", "prompt", "cron" ("new" for a new job), "host", "model".
    pub card: Option<(&'static str, String)>,
    /// Its blocks shut by a click on their head (by title).
    pub folded: std::collections::HashSet<&'static str>,
    /// A row's remove asked once ("mcp:<name>", "skill:<name>", "host:<id>", "project"): the
    /// second click does it.
    pub armed: Option<String>,
    pub error: Option<String>,
}

/// A skill a project's threads can use: its name, its front matter's description, where it lives
/// ("project": its own, "repo": the folder's .agents/skills, "general": the general agent's) and
/// its folder.
#[derive(Clone, Debug, Default)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub scope: String,
    pub dir: String,
}

/// One MCP server of the project. `checked` is the field that matters most: the agent only knows
/// a server once something has asked for its tools, so without it "no answer yet" and "answers
/// fine" look the same in the block.
#[derive(Clone, Debug)]
pub struct McpInfo {
    pub name: String,
    /// "http" or "stdio".
    pub kind: String,
    pub enabled: bool,
    /// The agent tried it at least once (a thread called it, or the person pressed Check).
    pub checked: bool,
    /// Its handshake is live now.
    pub connected: bool,
    pub tools: u64,
    pub error: Option<String>,
    /// It asks to sign in (OAuth) before it answers.
    pub needs_auth: bool,
    /// Its entry in mcp.json, as JSON text.
    pub spec: String,
}

impl SkillInfo {
    /// The skill's folder holds `rel` (a path inside it, "" its top).
    pub fn path(&self, rel: &str) -> String {
        if rel.is_empty() {
            self.dir.clone()
        } else {
            format!("{}/{rel}", self.dir.trim_end_matches('/'))
        }
    }
}

/// One access of the project, as its row shows it and its card edits it. A file host is the
/// project's own; an SSH one mirrors ~/.ssh/config.
#[derive(Clone, Debug, Default)]
pub struct Access {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub host: String,
    pub user: String,
    pub port: Option<u16>,
    pub path: String,
}

impl Access {
    /// What its row says after the kind: "user@host", or the address alone.
    pub fn at(&self) -> String {
        if self.user.is_empty() {
            self.host.clone()
        } else {
            format!("{}@{}", self.user, self.host)
        }
    }
}

/// A scheduled job of the project (the agent's `cron_jobs`): it runs `prompt` in a new thread of
/// the project on `schedule` ("every 1d", "0 9 * * 1-5", an ISO time for once).
#[derive(Clone, Debug, Default)]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub schedule: String,
    pub display: String,
    pub enabled: bool,
    pub next_run: Option<String>,
    pub last_run: Option<String>,
    pub last_status: Option<String>,
    pub last_output: String,
}

/// The agent a project's threads run with: its own (`project:<id>`), or this machine's for the
/// Host project. Its model, reasoning level and approvals live in that agent's folder, so a
/// project thinks and asks as it was set to, not as the machine was.
pub(super) fn env_agent(id: &str) -> String {
    if id == "general" {
        "default".into()
    } else {
        format!("project:{id}")
    }
}

/// A path segment for the agent's REST routes.
fn enc(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}

impl BenchApp {
    /// A project's pencil: its environment's thread (the latest, or a new one in the project).
    pub fn open_project_env(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.load_env(&id, window, cx);
        // "general": this Mac's own environment; its thread belongs to no project.
        let project = if id == "general" {
            String::new()
        } else {
            id.clone()
        };
        self.thread_project = project.clone();
        self.open_subject(format!("env:{id}"), cx);
        self.thread_project = project;
    }

    fn new_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> ProjectEnv {
        // One editor for all: the code editor, highlighted (Markdown, JSON), wrapped, line
        // numbers, no room under the last line; its height follows the text.
        let editor = |language: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            let e = cx.new(|cx| {
                EditorState::new(window, cx)
                    .line_number(true)
                    .soft_wrap(true)
                    .scroll_beyond_last_line(Some(0))
            });
            e.update(cx, |s, cx| s.set_highlighter(language, cx));
            e
        };
        let soul = editor("markdown", window, cx);
        let skill_text = editor("markdown", window, cx);
        let mcp_json = editor("json", window, cx);
        let prompt_text = editor("markdown", window, cx);
        let cron_prompt = editor("markdown", window, cx);
        let cron_name =
            cx.new(|cx| InputState::new(window, cx).placeholder("Its name: Weekly digest"));
        let cron_schedule = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("every 1d · every 30m · 0 9 * * 1-5 · 2026-11-01T09:00")
        });
        let name = cx.new(|cx| InputState::new(window, cx));
        let new_name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name"));
        let skill_form_name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name"));
        let tg_token =
            cx.new(|cx| InputState::new(window, cx).placeholder("Bot token from @BotFather"));
        let tg_users = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Telegram user ids who may write to it, comma-separated")
        });
        // An access's fields, filled from its row when its card opens.
        let h_name = cx.new(|cx| InputState::new(window, cx).placeholder("Its name"));
        let h_host = cx.new(|cx| InputState::new(window, cx).placeholder("example.com"));
        let h_port = cx.new(|cx| InputState::new(window, cx).placeholder("the usual port"));
        let h_user = cx.new(|cx| InputState::new(window, cx).placeholder("its user"));
        let h_password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("empty keeps the saved one")
        });
        let h_path = cx.new(|cx| {
            InputState::new(window, cx).placeholder("the folder to open in it, /var/www")
        });
        // A part typed in: its Save shows.
        fn dirty(this: &mut BenchApp, which: &str, ev: &InputEvent, cx: &mut Context<BenchApp>) {
            if let (InputEvent::Change, Some(e)) = (ev, this.project_env.as_mut()) {
                match which {
                    "soul" => e.soul_dirty = true,
                    "skill" => e.skill_dirty = true,
                    "telegram" => e.tg_dirty = true,
                    "prompt" => e.prompt_dirty = true,
                    "name" => e.name_dirty = true,
                    "cron" => e.cron_dirty = true,
                    "host" => e.host_dirty = true,
                    _ => e.mcp_dirty = true,
                }
                cx.notify();
            }
        }
        cx.subscribe(&soul, |this, _, ev, cx| dirty(this, "soul", ev, cx))
            .detach();
        cx.subscribe(&skill_text, |this, _, ev, cx| dirty(this, "skill", ev, cx))
            .detach();
        cx.subscribe(&mcp_json, |this, _, ev, cx| dirty(this, "mcp", ev, cx))
            .detach();
        cx.subscribe(&prompt_text, |this, _, ev, cx| {
            dirty(this, "prompt", ev, cx)
        })
        .detach();
        cx.subscribe(&name, |this, _, ev, cx| dirty(this, "name", ev, cx))
            .detach();
        cx.subscribe(&tg_token, |this, _, ev, cx| dirty(this, "telegram", ev, cx))
            .detach();
        cx.subscribe(&tg_users, |this, _, ev, cx| dirty(this, "telegram", ev, cx))
            .detach();
        cx.subscribe(&cron_prompt, |this, _, ev, cx| dirty(this, "cron", ev, cx))
            .detach();
        cx.subscribe(&cron_name, |this, _, ev, cx| dirty(this, "cron", ev, cx))
            .detach();
        cx.subscribe(&cron_schedule, |this, _, ev, cx| {
            dirty(this, "cron", ev, cx)
        })
        .detach();
        for f in [&h_name, &h_host, &h_port, &h_user, &h_password, &h_path] {
            cx.subscribe(f, |this, _, ev, cx| dirty(this, "host", ev, cx))
                .detach();
        }
        ProjectEnv {
            id: id.to_string(),
            dir: String::new(),
            soul,
            soul_open: false,
            soul_dirty: false,
            telegram: None,
            tg_open: false,
            tg_token,
            tg_users,
            tg_dirty: false,
            name,
            name_dirty: false,
            mcp: vec![],
            mcp_checking: false,
            mcp_open: None,
            mcp_json,
            mcp_dirty: false,
            prompts: vec![],
            prompt_open: None,
            prompt_text,
            prompt_dirty: false,
            skills: vec![],
            skill_open: None,
            skill_text,
            skill_dirty: false,
            skill_tree: vec![],
            skill_expanded: Default::default(),
            skill_folder: String::new(),
            skill_file: None,
            skill_form: None,
            skill_form_name,
            hosts: vec![],
            host_open: None,
            h_name,
            h_host,
            h_port,
            h_user,
            h_password,
            h_path,
            host_dirty: false,
            cron: vec![],
            cron_open: None,
            cron_name,
            cron_schedule,
            cron_prompt,
            cron_dirty: false,
            effort: String::new(),
            approvals: String::new(),
            folded: Default::default(),
            card: None,
            new_name,
            mcp_auth: Default::default(),
            armed: None,
            error: None,
        }
    }

    /// Read the environment for the block (again after the agent or a Save changed it); what is
    /// being typed is kept.
    pub fn load_env(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.project_env.as_ref().is_none_or(|e| e.id != id) {
            let env = self.new_env(id, window, cx);
            self.project_env = Some(env);
        }
        if self.demo {
            crate::demo::env(self, window, cx);
            return;
        }
        let Some((conn, eid)) = self.env_target(id) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!("/api/projects/{eid}/env");
        let asked = id.to_string();
        // How it runs, read from the same agent its threads run with.
        let conn2 = conn.clone();
        let agent = env_agent(&eid);
        let asked2 = asked.clone();
        self.background(
            window,
            cx,
            move || rest::profile_defaults(&conn2, &agent),
            move |this, r, _window, cx| {
                let Ok((effort, _, _, approvals, _)) = r else {
                    return;
                };
                let Some(e) = this.project_env.as_mut().filter(|e| e.id == asked2) else {
                    return;
                };
                e.effort = effort.unwrap_or_default();
                e.approvals = approvals;
                cx.notify();
            },
        );
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| {
                let Some(e) = this.project_env.as_mut().filter(|e| e.id == asked) else {
                    return;
                };
                match r {
                    Ok(v) => {
                        e.error = None;
                        apply(e, &v, window, cx);
                    }
                    Err(err) => e.error = Some(format!("{err:#}")),
                }
                cx.notify();
            },
        );
    }

    /// One of the two running choices — its reasoning level, its approvals — set for the
    /// project. A click applies it at once (a choice has no draft); the project's next threads
    /// start on it, a thread already begun keeps what it started with.
    pub fn env_set_run(
        &mut self,
        what: &'static str,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_ref() else {
            return;
        };
        let id = e.id.clone();
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let agent = env_agent(&eid);
        let body = match what {
            "reasoning" => json!({"config": {"agent": {"reasoning_effort": value}}}),
            _ => json!({"config": {"approvals": {"mode": value}}}),
        };
        let asked = id.clone();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "PUT", "/api/config", &agent, body),
            move |this, r, window, cx| match r {
                Ok(_) => this.load_env(&asked, window, cx),
                Err(err) => {
                    if let Some(e) = this.project_env.as_mut() {
                        e.error = Some(format!("{err:#}"));
                    }
                    cx.notify();
                }
            },
        );
    }

    /// Save what changed in the block: "soul", "mcp" (mcp.json), "skill" (the open one).
    pub fn env_save(&mut self, what: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_ref() else {
            return;
        };
        let id = e.id.clone();
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let (method, path, body) = match what {
            "soul" => (
                "PUT",
                format!("/api/projects/{eid}/soul"),
                json!({"text": e.soul.read(cx).value().to_string()}),
            ),
            "mcp" => {
                let Some(mut name) = e.mcp_open.clone() else {
                    return;
                };
                if name == "new" {
                    name = e.new_name.read(cx).value().trim().to_string();
                    if name.is_empty() {
                        self.toast(NotificationType::Error, "Name it first", window, cx);
                        return;
                    }
                }
                (
                    "PUT",
                    format!("/api/projects/{eid}/mcp/{}/spec", enc(&name)),
                    json!({"text": e.mcp_json.read(cx).value().to_string()}),
                )
            }
            "prompt" => {
                let Some(mut name) = e.prompt_open.clone() else {
                    return;
                };
                if name == "new" {
                    name = e.new_name.read(cx).value().trim().to_string();
                    if name.is_empty() {
                        self.toast(NotificationType::Error, "Name it first", window, cx);
                        return;
                    }
                }
                (
                    "PUT",
                    format!("/api/projects/{eid}/prompts/{}", enc(&name)),
                    json!({"text": e.prompt_text.read(cx).value().to_string()}),
                )
            }
            "cron" => {
                let Some(job) = e.cron_open.clone() else {
                    return;
                };
                let fields = json!({
                    "name": e.cron_name.read(cx).value().trim(),
                    "schedule": e.cron_schedule.read(cx).value().trim(),
                    "prompt": e.cron_prompt.read(cx).value().trim(),
                });
                if job == "new" {
                    ("POST", format!("/api/projects/{eid}/cron"), fields)
                } else {
                    (
                        "PUT",
                        format!("/api/cron/jobs/{}", enc(&job)),
                        json!({ "updates": fields }),
                    )
                }
            }
            // An access: PUT /api/hosts/<id>. The kind is not edited (change it by re-adding);
            // an empty password keeps the saved one, which is never read back to the client.
            "host" => {
                let Some(hid) = e.host_open.clone() else {
                    return;
                };
                let addr = e.h_host.read(cx).value().trim().to_string();
                if addr.is_empty() {
                    self.toast(
                        NotificationType::Error,
                        "The address is empty",
                        window,
                        cx,
                    );
                    return;
                }
                let empty = |s: &Entity<InputState>, cx: &mut Context<Self>| {
                    let v = s.read(cx).value().trim().to_string();
                    if v.is_empty() { None } else { Some(v) }
                };
                let mut body = json!({
                    "name": e.h_name.read(cx).value().trim(),
                    "kind": e.hosts.iter().find(|h| h.id == hid).map(|h| h.kind.clone()).unwrap_or_else(|| "ftp".into()),
                    "host": addr,
                    "port": empty(&e.h_port, cx).and_then(|p| p.parse::<u16>().ok()),
                    "user": e.h_user.read(cx).value().trim(),
                    "path": empty(&e.h_path, cx),
                });
                if let Some(pw) = empty(&e.h_password, cx) {
                    body["password"] = json!(pw);
                }
                ("PUT", format!("/api/hosts/{}", enc(&hid)), body)
            }
            // The name is the project's own row, on this Mac.
            "name" => (
                "PUT",
                format!("/api/projects/{id}"),
                json!({"name": e.name.read(cx).value().trim()}),
            ),
            "telegram" => {
                let token = e.tg_token.read(cx).value().trim().to_string();
                let users: Vec<String> = e
                    .tg_users
                    .read(cx)
                    .value()
                    .split(',')
                    .map(|u| u.trim().to_string())
                    .filter(|u| !u.is_empty())
                    .collect();
                let mut body = json!({"allowed_users": users});
                if !token.is_empty() {
                    body["token"] = json!(token);
                }
                ("PUT", format!("/api/projects/{eid}/telegram"), body)
            }
            _ => {
                let Some(name) = e.skill_open.clone() else {
                    return;
                };
                let text = e.skill_text.read(cx).value().to_string();
                if name == "new" {
                    (
                        "POST",
                        format!("/api/projects/{eid}/skills"),
                        json!({"text": text}),
                    )
                } else {
                    // The open file of the skill, wherever the skill lives.
                    let (Some(k), Some(rel)) = (
                        e.skills.iter().find(|k| k.name == name),
                        e.skill_file.as_ref(),
                    ) else {
                        return;
                    };
                    (
                        "POST",
                        "/api/fs/write-text".to_string(),
                        json!({"path": k.path(rel), "content": text}),
                    )
                }
            }
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, method, &path, &profile, body),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.project_env.as_mut() {
                        // A new one is made: its form closes, it is in the list.
                        if e.card.as_ref().is_some_and(|(_, n)| n == "new") && what != "cron" {
                            e.card = None;
                            e.mcp_open = None;
                            e.skill_open = None;
                            e.prompt_open = None;
                        }
                        match what {
                            "soul" => e.soul_dirty = false,
                            "mcp" => e.mcp_dirty = false,
                            "telegram" => e.tg_dirty = false,
                            "prompt" => e.prompt_dirty = false,
                            "name" => e.name_dirty = false,
                            "cron" => {
                                e.cron_dirty = false;
                                // A new job: its form closes, the job is in the list.
                                if e.cron_open.as_deref() == Some("new") {
                                    e.cron_open = None;
                                    e.card = None;
                                }
                            }
                            "host" => {
                                e.host_dirty = false;
                                // Its card closes: the row shows the new name and address.
                                e.host_open = None;
                                e.card = None;
                            }
                            _ => e.skill_dirty = false,
                        }
                    }
                    this.load_env(&id, window, cx);
                    if what == "name" {
                        this.load_projects(window, cx);
                    }
                    // An access's name and address are shown in /ftp and in its card too.
                    if what == "host" {
                        this.load_remote_hosts(window, cx);
                    }
                    if what == "prompt" {
                        this.load_prompts(window, cx);
                    }
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// Revert: what is saved, back in the field.
    pub fn env_revert(&mut self, what: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        match what {
            "soul" => e.soul_dirty = false,
            "mcp" => e.mcp_dirty = false,
            "telegram" => e.tg_dirty = false,
            "prompt" => {
                e.prompt_dirty = false;
                if let Some(name) = e.prompt_open.take() {
                    return self.env_open("prompt", name, window, cx);
                }
            }
            "name" => e.name_dirty = false,
            "cron" => {
                e.cron_dirty = false;
                if let Some(job) = e.cron_open.take() {
                    if job != "new" {
                        return self.env_open_cron(job, window, cx);
                    }
                }
            }
            "host" => {
                e.host_dirty = false;
                if let Some(hid) = e.host_open.take() {
                    return self.env_open_host(hid, window, cx);
                }
            }
            _ => {
                e.skill_dirty = false;
                if let Some(rel) = e.skill_file.clone() {
                    return self.skill_open_file(rel, window, cx);
                }
            }
        }
        let id = e.id.clone();
        self.load_env(&id, window, cx);
    }

    /// A row's Remove, twice: armed, then done. `what`: "mcp", "skill", "host" (a file host is
    /// deleted — it is this project's own; an SSH access only leaves the project), "project".
    pub fn env_remove(
        &mut self,
        what: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let key = format!("{what}:{name}");
        if e.armed.as_deref() != Some(&key) {
            e.armed = Some(key);
            cx.notify();
            return;
        }
        e.armed = None;
        let id = e.id.clone();
        if what == "project" {
            self.project_env = None;
            self.delete_project(id, window, cx);
            self.new_session(cx);
            self.thread_project = String::new();
            return;
        }
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let path = match what {
            "mcp" => format!("/api/projects/{eid}/mcp/{}", enc(&name)),
            "skill" => format!("/api/projects/{eid}/skills/{}", enc(&name)),
            "prompt" => format!("/api/projects/{eid}/prompts/{}", enc(&name)),
            "cron" => format!("/api/cron/jobs/{}", enc(&name)),
            _ => format!("/api/projects/{eid}/hosts/{}", enc(&name)),
        };
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "DELETE", &path, &profile, Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    // What was removed had its card open.
                    if let Some(e) = this.project_env.as_mut() {
                        e.card = None;
                    }
                    this.load_env(&id, window, cx);
                    this.load_remote_hosts(window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// An MCP server's or a prompt's row: its text opens under it to edit (again: closes).
    pub fn env_open(
        &mut self,
        what: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let (open, editor, dirty) = if what == "mcp" {
            (&mut e.mcp_open, &e.mcp_json, &mut e.mcp_dirty)
        } else {
            (&mut e.prompt_open, &e.prompt_text, &mut e.prompt_dirty)
        };
        if open.as_deref() == Some(&name) && !*dirty {
            *open = None;
            cx.notify();
            return;
        }
        *open = Some(name.clone());
        *dirty = false;
        let editor = editor.clone();
        if what == "mcp" {
            let spec = e
                .mcp
                .iter()
                .find(|m| m.name == name)
                .map(|m| m.spec.clone())
                .unwrap_or_default();
            editor.update(cx, |s, cx| s.set_value(spec, window, cx));
            cx.notify();
            return;
        }
        let pid = e.id.clone();
        let Some((conn, eid)) = self.env_target(&pid) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!("/api/projects/{eid}/prompts");
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |_, r, window, cx| {
                if let Ok(v) = r {
                    let text = v["prompts"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|p| p["name"].as_str() == Some(&name))
                        .and_then(|p| p["text"].as_str())
                        .unwrap_or_default()
                        .to_string();
                    editor.update(cx, |s, cx| s.set_value(text, window, cx));
                }
                cx.notify();
            },
        );
    }

    /// A skill's row: its folder opens under it as a tree, with its SKILL.md open to read and
    /// edit (again: closes).
    pub fn env_open_skill(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if e.skill_open.as_deref() == Some(&name) && !e.skill_dirty {
            e.skill_open = None;
            cx.notify();
            return;
        }
        e.skill_open = Some(name.clone());
        e.skill_dirty = false;
        e.skill_tree.clear();
        e.skill_expanded.clear();
        e.skill_folder.clear();
        e.skill_form = None;
        e.skill_file = Some("SKILL.md".into());
        if self.demo {
            let text = format!(
                "---\nname: {name}\ndescription: A made-up skill of the demo.\n---\n\nWhat to do, step by step."
            );
            e.skill_tree = vec![
                ("references".into(), true),
                ("references/style.md".into(), false),
                ("scripts".into(), true),
                ("SKILL.md".into(), false),
            ];
            e.skill_expanded.insert("references".into());
            e.skill_text
                .update(cx, |s, cx| s.set_value(text, window, cx));
            e.skill_dirty = false;
            cx.notify();
            return;
        }
        self.skill_load_tree(window, cx);
        self.skill_open_file("SKILL.md".into(), window, cx);
    }

    /// The open skill, as the environment lists it.
    fn open_skill(&self) -> Option<SkillInfo> {
        let e = self.project_env.as_ref()?;
        let name = e.skill_open.as_ref()?;
        e.skills.iter().find(|k| &k.name == name).cloned()
    }

    /// Read the open skill's folder again (after a file or folder is made, renamed or removed).
    pub fn skill_load_tree(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skill) = self.open_skill() else {
            return;
        };
        let pid = self
            .project_env
            .as_ref()
            .map(|e| e.id.clone())
            .unwrap_or_default();
        let Some((conn, _)) = self.env_target(&pid) else {
            return;
        };
        let profile = self.current_profile();
        let path = format!("/api/fs/tree?path={}", enc(&skill.dir));
        self.background(
            window,
            cx,
            move || rest::get_raw(&conn, &profile, &path),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    if let Some(e) = this
                        .project_env
                        .as_mut()
                        .filter(|e| e.skill_open.as_deref() == Some(&skill.name))
                    {
                        e.skill_tree = v["entries"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|t| {
                                (
                                    t["path"].as_str().unwrap_or_default().to_string(),
                                    t["dir"].as_bool().unwrap_or(false),
                                )
                            })
                            .collect();
                    }
                    cx.notify();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A file of the open skill opens under its tree, highlighted as its kind.
    pub fn skill_open_file(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skill) = self.open_skill() else {
            return;
        };
        let pid = self
            .project_env
            .as_ref()
            .map(|e| e.id.clone())
            .unwrap_or_default();
        let Some((conn, _)) = self.env_target(&pid) else {
            return;
        };
        let profile = self.current_profile();
        let path = skill.path(&rel);
        self.background(
            window,
            cx,
            move || rest::read_text(&conn, &profile, &path),
            move |this, r, window, cx| {
                match r {
                    Ok(t) if t.binary => this.toast(
                        NotificationType::Info,
                        "Not a text file: show it in Finder",
                        window,
                        cx,
                    ),
                    Ok(t) => {
                        if let Some(e) = this
                            .project_env
                            .as_mut()
                            .filter(|e| e.skill_open.as_deref() == Some(&skill.name))
                        {
                            let language = crate::lang::language_for_text(&rel, &t.text);
                            e.skill_file = Some(rel);
                            e.skill_text.update(cx, |s, cx| {
                                s.set_highlighter(language, cx);
                                s.set_value(t.text, window, cx)
                            });
                            e.skill_dirty = false;
                        }
                    }
                    Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
                }
                cx.notify();
            },
        );
    }

    /// A row of the skill's tree: a folder opens or shuts (and is where a new file goes), a file
    /// opens under the tree.
    pub fn skill_entry(
        &mut self,
        rel: String,
        dir: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if dir {
            if !e.skill_expanded.remove(&rel) {
                e.skill_expanded.insert(rel.clone());
                e.skill_folder = rel;
            } else {
                e.skill_folder = parent_rel(&rel);
            }
            cx.notify();
            return;
        }
        if e.skill_dirty {
            self.toast(
                NotificationType::Info,
                "Save or revert the open file first",
                window,
                cx,
            );
            return;
        }
        e.skill_folder = parent_rel(&rel);
        self.skill_open_file(rel, window, cx);
    }

    /// "New file", "New folder" (in the folder last opened), or a row's rename: the field that
    /// names it, under the tree.
    pub fn skill_form(
        &mut self,
        what: &'static str,
        target: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let name = if what == "rename" {
            target.rsplit('/').next().unwrap_or_default().to_string()
        } else {
            String::new()
        };
        e.skill_form_name.update(cx, |s, cx| {
            s.set_value(name, window, cx);
            s.focus(window, cx)
        });
        e.skill_form = Some((what, target));
        cx.notify();
    }

    /// Make what the field names (or rename), then read the tree again; a new file opens.
    pub fn skill_form_done(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(skill), Some(e)) = (self.open_skill(), self.project_env.as_ref()) else {
            return;
        };
        let Some((what, target)) = e.skill_form.clone() else {
            return;
        };
        let name = e
            .skill_form_name
            .read(cx)
            .value()
            .trim()
            .trim_matches('/')
            .to_string();
        if name.is_empty()
            || name.split('/').any(|p| p.is_empty() || p == "..")
            || (what == "rename" && name.contains('/'))
        {
            self.toast(
                NotificationType::Error,
                "A name (a file may go in a folder: a/b.md)",
                window,
                cx,
            );
            return;
        }
        let rel = match what {
            "rename" => {
                let up = parent_rel(&target);
                if up.is_empty() {
                    name.clone()
                } else {
                    format!("{up}/{name}")
                }
            }
            _ if target.is_empty() => name.clone(),
            _ => format!("{target}/{name}"),
        };
        let Some((conn, _)) = self.env_target(&e.id) else {
            return;
        };
        let profile = self.current_profile();
        let (path, from) = (skill.path(&rel), skill.path(&target));
        let rel2 = rel.clone();
        self.background(
            window,
            cx,
            move || match what {
                "folder" => rest::send_json(
                    &conn,
                    "POST",
                    "/api/fs/mkdir",
                    &profile,
                    json!({"path": path}),
                )
                .map(|_| ()),
                "file" => rest::write_text(&conn, &profile, &path, ""),
                _ => rest::send_json(
                    &conn,
                    "POST",
                    "/api/fs/rename",
                    &profile,
                    json!({"from": from, "to": path}),
                )
                .map(|_| ()),
            },
            move |this, r, window, cx| match r {
                Ok(()) => {
                    if let Some(e) = this.project_env.as_mut() {
                        e.skill_form = None;
                        let up = parent_rel(&rel2);
                        if !up.is_empty() {
                            e.skill_expanded.insert(up);
                        }
                        // What was renamed: the open file (or a folder above it) moves with it.
                        if what == "rename" {
                            if let Some(f) = e.skill_file.clone() {
                                if f == target {
                                    e.skill_file = Some(rel2.clone());
                                } else if let Some(rest) = f.strip_prefix(&format!("{target}/")) {
                                    e.skill_file = Some(format!("{rel2}/{rest}"));
                                }
                            }
                        }
                    }
                    this.skill_load_tree(window, cx);
                    if what == "file" {
                        this.skill_open_file(rel2, window, cx);
                    }
                    let id = this
                        .project_env
                        .as_ref()
                        .map(|e| e.id.clone())
                        .unwrap_or_default();
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A tree row's trash, twice: armed, then the file or the folder (with what is in it) goes.
    pub fn skill_delete(&mut self, rel: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(skill) = self.open_skill() else {
            return;
        };
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let key = format!("skillfile:{rel}");
        if e.armed.as_deref() != Some(&key) {
            e.armed = Some(key);
            cx.notify();
            return;
        }
        e.armed = None;
        let id = e.id.clone();
        let Some((conn, _)) = self.env_target(&id) else {
            return;
        };
        let profile = self.current_profile();
        let path = skill.path(&rel);
        self.background(
            window,
            cx,
            move || {
                rest::send_json(
                    &conn,
                    "POST",
                    "/api/fs/delete",
                    &profile,
                    json!({"path": path}),
                )
            },
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if let Some(e) = this.project_env.as_mut() {
                        if e.skill_file
                            .as_ref()
                            .is_some_and(|f| *f == rel || f.starts_with(&format!("{rel}/")))
                        {
                            e.skill_file = None;
                            e.skill_dirty = false;
                        }
                    }
                    this.skill_load_tree(window, cx);
                    let id = this
                        .project_env
                        .as_ref()
                        .map(|e| e.id.clone())
                        .unwrap_or_default();
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A folder or file shown in Finder (this Mac's projects only).
    pub fn reveal_in_finder(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
        {
            self.toast(NotificationType::Error, format!("{err:#}"), window, cx);
        }
    }

    /// A row's click: its card opens under the block (again: it closes); one card at a time.
    pub fn env_card(
        &mut self,
        kind: &'static str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        let again = e
            .card
            .as_ref()
            .is_some_and(|(k, n)| *k == kind && *n == name);
        e.soul_open = false;
        e.tg_open = false;
        e.mcp_open = None;
        e.skill_open = None;
        e.skill_form = None;
        e.prompt_open = None;
        e.cron_open = None;
        e.host_open = None;
        e.armed = None;
        if again {
            e.card = None;
            cx.notify();
            return;
        }
        e.card = Some((kind, name.clone()));
        // A new one: an empty form, its text pasted (or written), then Save.
        if name == "new" && matches!(kind, "mcp" | "skill" | "prompt") {
            let (editor, template) = match kind {
                "mcp" => (e.mcp_json.clone(), "{\n  \"url\": \"https://\"\n}\n"),
                "skill" => (e.skill_text.clone(), "---\nname: \ndescription: \n---\n\n"),
                _ => (e.prompt_text.clone(), ""),
            };
            match kind {
                "mcp" => e.mcp_open = Some(name),
                "skill" => e.skill_open = Some(name),
                _ => e.prompt_open = Some(name),
            }
            editor.update(cx, |s, cx| s.set_value(template, window, cx));
            e.new_name.update(cx, |s, cx| s.set_value("", window, cx));
            e.new_name.update(cx, |s, cx| s.focus(window, cx));
            cx.notify();
            return;
        }
        match kind {
            "soul" => e.soul_open = true,
            "telegram" => e.tg_open = true,
            "mcp" | "prompt" => self.env_open(kind, name, window, cx),
            "skill" => self.env_open_skill(name, window, cx),
            "cron" => self.env_open_cron(name, window, cx),
            "host" => self.env_open_host(name, window, cx),
            _ => {}
        }
        cx.notify();
    }

    /// A job's row: its name, schedule and prompt open under it to edit (again: closes); "new":
    /// an empty form for a new job.
    pub fn env_open_cron(&mut self, job: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if e.cron_open.as_deref() == Some(&job) && !e.cron_dirty {
            e.cron_open = None;
            cx.notify();
            return;
        }
        let j = e
            .cron
            .iter()
            .find(|j| j.id == job)
            .cloned()
            .unwrap_or_default();
        e.cron_name
            .update(cx, |s, cx| s.set_value(j.name.clone(), window, cx));
        e.cron_schedule
            .update(cx, |s, cx| s.set_value(j.schedule.clone(), window, cx));
        e.cron_prompt
            .update(cx, |s, cx| s.set_value(j.prompt.clone(), window, cx));
        e.cron_open = Some(job.clone());
        e.cron_dirty = false;
        if job == "new" {
            e.cron_name.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    /// An access's row: its fields open under it to edit (again: closes). The saved password is
    /// never read back, so its field starts empty and empty means "keep it".
    pub fn env_open_host(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = self.project_env.as_mut() else {
            return;
        };
        if e.host_open.as_deref() == Some(&id) && !e.host_dirty {
            e.host_open = None;
            cx.notify();
            return;
        }
        let h = e.hosts.iter().find(|h| h.id == id).cloned().unwrap_or_default();
        e.h_name.update(cx, |s, cx| s.set_value(h.name, window, cx));
        e.h_host.update(cx, |s, cx| s.set_value(h.host, window, cx));
        e.h_port.update(cx, |s, cx| {
            s.set_value(h.port.map(|p| p.to_string()).unwrap_or_default(), window, cx)
        });
        e.h_user.update(cx, |s, cx| s.set_value(h.user, window, cx));
        e.h_password
            .update(cx, |s, cx| s.set_value("", window, cx));
        e.h_path.update(cx, |s, cx| s.set_value(h.path, window, cx));
        e.host_open = Some(id);
        e.host_dirty = false;
        cx.notify();
    }

    /// A job's "Run now", "Pause", "Resume".
    pub fn env_cron(
        &mut self,
        job: String,
        action: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.project_env.as_ref().map(|e| e.id.clone()) else {
            return;
        };
        let Some((conn, _)) = self.env_target(&id) else {
            return;
        };
        let path = format!("/api/cron/jobs/{}/{action}", enc(&job));
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", &path, &profile, Value::Null),
            move |this, r, window, cx| match r {
                Ok(_) => {
                    if action == "trigger" {
                        this.toast(
                            NotificationType::Info,
                            "Started: its thread shows in the project",
                            window,
                            cx,
                        );
                    }
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// Ask the project's MCP servers now: the agent shakes hands with every one of them and the
    /// rows say what came back. A server only tells you what it is when you ask it, so this is
    /// the difference between "unknown" and "working"; the agent answers when the slowest has had
    /// its say, so this is the one environment job allowed to take a minute.
    pub fn env_check_mcp(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // One check at a time: a second reconnect while the first is in flight would answer out
        // of order and show a stale set.
        if self.project_env.as_ref().is_some_and(|e| e.mcp_checking) {
            return;
        }
        let Some(id) = self.project_env.as_ref().map(|e| e.id.clone()) else {
            return;
        };
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        if let Some(e) = self.project_env.as_mut() {
            e.mcp_checking = true;
        }
        cx.notify();
        let path = format!("/api/projects/{eid}/mcp/check");
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json_secs(&conn, "POST", &path, &profile, Value::Null, 90),
            move |this, r, window, cx| {
                let mut note = None;
                if let Some(e) = this.project_env.as_mut().filter(|e| e.id == id) {
                    e.mcp_checking = false;
                    match &r {
                        Ok(v) => {
                            apply_mcp(e, v);
                            note = Some(crate::app::mcp_status::mcp_tally(&e.mcp, false).1);
                        }
                        Err(err) => note = Some(format!("{err:#}")),
                    }
                }
                if let Some(text) = note {
                    this.toast(
                        if r.is_ok() {
                            NotificationType::Info
                        } else {
                            NotificationType::Error
                        },
                        text,
                        window,
                        cx,
                    );
                }
                cx.notify();
            },
        );
    }

    /// Sign in to an MCP server (OAuth): its page opens in the browser; the agent connects it once
    /// the browser comes back, and the block reads the environment again until it shows connected.
    pub fn env_authorize(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.project_env.as_ref().map(|e| e.id.clone()) else {
            return;
        };
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let path = format!("/api/projects/{eid}/mcp/{}/authorize", enc(&name));
        let profile = self.current_profile();
        self.background(
            window,
            cx,
            move || rest::send_json(&conn, "POST", &path, &profile, Value::Null),
            move |this, r, window, cx| match r {
                Ok(v) => {
                    match v["url"].as_str().filter(|u| !u.is_empty()) {
                        Some(url) => {
                            cx.open_url(url);
                            this.toast(
                                NotificationType::Info,
                                "Sign in in the browser: the server connects when you are back",
                                window,
                                cx,
                            );
                        }
                        None => this.toast(NotificationType::Success, "Connected", window, cx),
                    }
                    // Read again every 3 s for 3 minutes, until it no longer asks.
                    cx.spawn_in(window, async move |this, cx| {
                        for _ in 0..60 {
                            cx.background_executor()
                                .timer(std::time::Duration::from_secs(3))
                                .await;
                            let more = this
                                .update_in(cx, |this, window, cx| {
                                    let waiting = this
                                        .project_env
                                        .as_ref()
                                        .is_some_and(|e| e.id == id && e.mcp_auth.contains(&name));
                                    if waiting {
                                        this.load_env(&id, window, cx);
                                    }
                                    waiting
                                })
                                .unwrap_or(false);
                            if !more {
                                break;
                            }
                        }
                    })
                    .detach();
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }

    /// A section's "+ Add": the start of the request in the composer, the caret after it and the
    /// thread scrolled down to it, for the person to finish and the agent to do.
    pub fn ask_in_composer(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let end = gpui_kit::component::input::Position::new(0, text.chars().count() as u32);
        self.composer.update(cx, |s, cx| {
            s.set_value(text.to_string(), window, cx);
            s.set_cursor_position(end, window, cx);
        });
        self.jump_to_latest(cx);
    }
}

/// The folder a path inside a skill is in ("" its top).
fn parent_rel(rel: &str) -> String {
    rel.rsplit_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default()
}

/// The MCP part of the agent's answer: the whole environment read and a Check both carry the same
/// `mcp` array, so one parser serves them.
pub fn apply_mcp(e: &mut ProjectEnv, v: &Value) {
    e.mcp = v["mcp"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| McpInfo {
            name: m["name"].as_str().unwrap_or_default().to_string(),
            kind: m["kind"].as_str().unwrap_or_default().to_string(),
            enabled: m["enabled"].as_bool().unwrap_or(true),
            checked: m["checked"].as_bool().unwrap_or(false),
            connected: m["connected"].as_bool().unwrap_or(false),
            tools: m["tools"].as_u64().unwrap_or(0),
            error: m["error"].as_str().map(str::to_string),
            needs_auth: m["needs_auth"].as_bool().unwrap_or(false),
            spec: m["spec"].as_str().unwrap_or("{}").to_string(),
        })
        .collect();
    e.mcp_auth = e
        .mcp
        .iter()
        .filter(|m| m.needs_auth)
        .map(|m| m.name.clone())
        .collect();
}

/// The agent's answer into the block; fields being typed in keep their text.
pub fn apply(e: &mut ProjectEnv, v: &Value, window: &mut Window, cx: &mut App) {
    e.dir = v["dir"].as_str().unwrap_or_default().to_string();
    apply_mcp(e, v);
    e.skills = v["skills"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| {
            let s = |f: &str| k[f].as_str().unwrap_or_default().to_string();
            SkillInfo {
                name: s("name"),
                description: s("description"),
                scope: s("scope"),
                dir: s("dir"),
            }
        })
        .collect();
    e.hosts = v["hosts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|h| {
            let s = |k: &str| h[k].as_str().unwrap_or_default().to_string();
            Access {
                id: s("id"),
                name: s("name"),
                kind: s("kind"),
                host: s("host"),
                user: s("user"),
                port: h["port"].as_u64().map(|p| p as u16),
                path: s("path"),
            }
        })
        .collect();
    e.telegram = v["telegram"].as_object().map(|t| {
        (
            t.get("username")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            t.get("allowed_users")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|u| u.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            t.get("token")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    if !e.tg_dirty {
        let users = e
            .telegram
            .as_ref()
            .map(|t| t.1.join(", "))
            .unwrap_or_default();
        e.tg_users
            .update(cx, |s, cx| s.set_value(users, window, cx));
        let token = e.telegram.as_ref().map(|t| t.2.clone()).unwrap_or_default();
        e.tg_token
            .update(cx, |s, cx| s.set_value(token, window, cx));
    }
    if !e.soul_dirty {
        let text = v["soul"].as_str().unwrap_or_default().to_string();
        e.soul.update(cx, |s, cx| s.set_value(text, window, cx));
    }
    e.cron = v["cron"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|j| {
            let s = |k: &str| j[k].as_str().unwrap_or_default().to_string();
            let o = |k: &str| j[k].as_str().filter(|s| !s.is_empty()).map(str::to_string);
            CronJob {
                id: s("id"),
                name: s("name"),
                prompt: s("prompt"),
                schedule: j["schedule"]["expr"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                display: s("schedule_display"),
                enabled: j["enabled"].as_bool().unwrap_or(true),
                next_run: o("next_run_at"),
                last_run: o("last_run_at"),
                last_status: o("last_status"),
                last_output: s("last_output"),
            }
        })
        .collect();
    e.prompts = v["prompts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p.as_str().map(str::to_string))
        .collect();
    if !e.name_dirty {
        let name = v["project"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        e.name.update(cx, |s, cx| s.set_value(name, window, cx));
    }
}
