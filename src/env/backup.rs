//! The project's environment as one file: exported for a backup or to move it, imported into
//! this project from such a file (the agent's `modules/backup.ts` packs and unpacks it). Two rows
//! of its Settings, before Remove; an import asks first, as a Mac alert.

use gpui_kit::component::notification::NotificationType;
use gpui_kit::*;
use serde_json::Value;

use super::ProjectEnv;
use super::section::{Section, SettingsForm};
use crate::app::BenchApp;
use crate::client::rest;
use bench_ui::settings_field;

pub struct Backup;

impl Section for Backup {
    fn key(&self) -> &'static str {
        "backup"
    }
}

impl SettingsForm for Backup {
    fn block(&self, _general: bool) -> &'static str {
        "Backup"
    }

    fn form(
        &self,
        _app: &BenchApp,
        e: &ProjectEnv,
        cx: &mut Context<BenchApp>,
    ) -> Option<Vec<AnyElement>> {
        let (p1, p2) = (e.id.clone(), e.id.clone());
        let button = |id: &'static str, label: &'static str, cx: &mut Context<BenchApp>| {
            div()
                .flex()
                .justify_end()
                .child(bench_ui::form_action(id, label, false, cx))
        };
        Some(vec![
            settings_field(
                "Export",
                "Soul, prompts, skills, servers, schedule and bot, in one file",
                button("env-export", "Export…", cx)
                    .id("env-export-row")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.env_export(p1.clone(), window, cx)
                    }))
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
            settings_field(
                "Import",
                "From an exported file; the same names here are replaced",
                button("env-import", "Import…", cx)
                    .id("env-import-row")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.env_import(p2.clone(), window, cx)
                    }))
                    .into_any_element(),
                cx,
            )
            .into_any_element(),
        ])
    }
}

/// An exported environment read from `path`; None: it is not one.
pub(crate) fn read(path: &std::path::Path) -> Option<Value> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    (v["kind"] == "bench.environment").then_some(v)
}

/// What a file holds, as a sentence: "its soul, 12 skills, 4 prompts, 5 MCP servers".
pub(crate) fn contents(v: &Value) -> String {
    let files = v["files"].as_object();
    let has = |k: &str| files.is_some_and(|f| f.contains_key(k));
    let count = |prefix: &str, skill: bool| -> usize {
        let mut names: Vec<&str> = files
            .into_iter()
            .flat_map(|f| f.keys())
            .filter_map(|k| k.strip_prefix(prefix))
            .filter(|k| skill || k.ends_with(".md"))
            .map(|k| if skill { k.split('/').next().unwrap_or(k) } else { k })
            .collect();
        names.sort_unstable();
        names.dedup();
        names.len()
    };
    let servers = |file: &str, key: &str| -> usize {
        files
            .and_then(|f| f.get(file))
            .and_then(|f| f["text"].as_str())
            .and_then(|t| serde_json::from_str::<Value>(t).ok())
            .and_then(|j| j[key].as_object().map(|o| o.len()))
            .unwrap_or(0)
    };
    let n = |k: usize, one: &str, many: &str| match k {
        0 => None,
        1 => Some(format!("1 {one}")),
        k => Some(format!("{k} {many}")),
    };
    let parts: Vec<String> = [
        has("SYSTEM.md").then(|| "its soul".to_string()),
        n(count("skills/", true), "skill", "skills"),
        n(count("prompts/", false), "prompt", "prompts"),
        n(servers("mcp.json", "mcpServers"), "MCP server", "MCP servers"),
        n(servers("lsp.json", "servers"), "language server", "language servers"),
        n(
            v["cron"].as_array().map_or(0, |a| a.len()),
            "scheduled job",
            "scheduled jobs",
        ),
        v["telegram"]["token"]
            .as_str()
            .map(|_| "its Telegram bot".to_string()),
    ]
    .into_iter()
    .flatten()
    .collect();
    let said = match parts.as_slice() {
        [] => "nothing but its settings".into(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    // A sentence of its own: "Its soul, 12 skills…".
    let mut c = said.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

impl BenchApp {
    /// "Export…": where to save the file, then the agent's environment of the project `id` there.
    pub fn env_export(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let name = self.project_name(&id).replace(['/', ':'], "-");
        let day = chrono::Local::now().format("%Y-%m-%d");
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let picked = cx.prompt_for_new_path(
            &home.join("Documents"),
            Some(&format!("{name} environment {day}.json")),
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(dest))) = picked.await else {
                return;
            };
            let path = format!("/api/projects/{}/export", super::enc(&eid));
            let _ = this.update_in(cx, |this, window, cx| {
                this.background(
                    window,
                    cx,
                    move || {
                        let v = rest::get(&conn, &path)?;
                        std::fs::write(&dest, serde_json::to_string_pretty(&v)?)?;
                        Ok((dest, v))
                    },
                    |this, r, window, cx| match r {
                        Ok((dest, v)) => {
                            let file = dest
                                .file_name()
                                .map(|f| f.to_string_lossy().to_string())
                                .unwrap_or_default();
                            let mut said = format!("Exported to {file}. {}", contents(&v));
                            if let Some(n) = v["skipped"].as_array().filter(|a| !a.is_empty()) {
                                said.push_str(&format!(
                                    ". {} files over 8 MB left out",
                                    n.len()
                                ));
                            }
                            this.toast(NotificationType::Success, said, window, cx)
                        }
                        Err(err) => {
                            this.toast(NotificationType::Error, format!("{err:#}"), window, cx)
                        }
                    },
                );
            });
        })
        .detach();
    }

    /// "Import…": a file picked, what it holds said in an alert, then it goes into the project
    /// `id`'s environment.
    pub fn env_import(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let Some(file) = read(&path) else {
                let _ = this.update_in(cx, |this, window, cx| this.not_environment(window, cx));
                return;
            };
            let Ok(answer) = this.update_in(cx, |this, window, cx| {
                let from = file["name"].as_str().unwrap_or("another project");
                window.prompt(
                    PromptLevel::Info,
                    &format!("Import “{from}” into “{}”?", this.project_name(&id)),
                    Some(&format!(
                        "{}. What has the same name here is replaced.",
                        contents(&file)
                    )),
                    &[
                        PromptButton::Ok("Import".into()),
                        PromptButton::Cancel("Cancel".into()),
                    ],
                    cx,
                )
            }) else {
                return;
            };
            if answer.await != Ok(0) {
                return;
            }
            let _ = this.update_in(cx, |this, window, cx| this.env_import_file(id, file, window, cx));
        })
        .detach();
    }

    /// A file picked that is not an exported environment.
    pub(crate) fn not_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toast(
            NotificationType::Error,
            "This is not an environment Bench exported",
            window,
            cx,
        )
    }

    /// The file `file` sent to the agent of the project `id`; its environment read again.
    pub(crate) fn env_import_file(&mut self, id: String, file: Value, window: &mut Window, cx: &mut Context<Self>) {
        let Some((conn, eid)) = self.env_target(&id) else {
            return;
        };
        let path = format!("/api/projects/{}/import", super::enc(&eid));
        // A bot started and jobs armed besides the files: more than a plain save.
        self.background(
            window,
            cx,
            move || rest::send_json_secs(&conn, "POST", &path, file, 120),
            move |this, r, window, cx| match r {
                Ok(done) => {
                    let mut said = "Imported".to_string();
                    if let Some(from) = done["bot_moved_from"].as_str() {
                        said.push_str(&format!(
                            ". The Telegram bot moved here from “{from}”"
                        ));
                    }
                    this.toast(NotificationType::Success, said, window, cx);
                    this.load_env(&id, window, cx);
                }
                Err(err) => this.toast(NotificationType::Error, format!("{err:#}"), window, cx),
            },
        );
    }
}
