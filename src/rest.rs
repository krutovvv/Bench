//! Blocking REST calls to the gateway's dashboard API. Call them off the UI thread.

use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use base64::Engine as _;
use serde_json::{Value, json};

use crate::connection::Connection;
use crate::model::SkillRow;

fn url(conn: &Connection, path: &str, profile: &str) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    if profile.is_empty() || profile == "default" {
        format!("{}{path}", conn.base)
    } else {
        format!("{}{path}{sep}profile={profile}", conn.base)
    }
}

fn get(conn: &Connection, path: &str, profile: &str) -> Result<Value> {
    let (h, v) = conn.rest_auth(profile)?;
    let resp = ureq::get(&url(conn, path, profile))
        .set(h, &v)
        .timeout(Duration::from_secs(30))
        .call();
    Ok(resp.map_err(|e| anyhow!("{path}: {e}"))?.into_json()?)
}

fn send(
    conn: &Connection,
    method: &str,
    path: &str,
    profile: &str,
    body: Value,
    timeout: u64,
) -> Result<Value> {
    let (h, v) = conn.rest_auth(profile)?;
    let resp = ureq::request(method, &url(conn, path, profile))
        .set(h, &v)
        .timeout(Duration::from_secs(timeout))
        .send_json(body);
    match resp {
        Ok(r) => Ok(r.into_json()?),
        Err(ureq::Error::Status(code, r)) => {
            let body: Value = r.into_json().unwrap_or(Value::Null);
            Err(anyhow!(
                "{code}: {}",
                body["detail"].as_str().unwrap_or("request failed")
            ))
        }
        Err(e) => Err(anyhow!("{path}: {e}")),
    }
}

pub fn skills(conn: &Connection, profile: &str) -> Result<Vec<SkillRow>> {
    let v = get(conn, "/api/skills", profile)?;
    let mut rows: Vec<SkillRow> = v
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| SkillRow {
            name: s["name"].as_str().unwrap_or_default().into(),
            category: s["category"]
                .as_str()
                .filter(|c| !c.is_empty())
                .unwrap_or("other")
                .into(),
            description: s["description"].as_str().unwrap_or_default().into(),
            enabled: s["enabled"].as_bool().unwrap_or(true),
        })
        .collect();
    rows.sort_by(|a, b| (&a.category, &a.name).cmp(&(&b.category, &b.name)));
    Ok(rows)
}

/// Owners of Telegram sessions: (session id, telegram user id, display name).
pub fn telegram_owners(conn: &Connection, profile: &str) -> Result<Vec<(String, String, String)>> {
    let v = get(
        conn,
        "/api/sessions?source=telegram&limit=100&order=recent",
        profile,
    )?;
    Ok(v["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let user = s["user_id"].as_str().or(s["chat_id"].as_str())?.to_string();
            let name = s["display_name"]
                .as_str()
                .filter(|n| !n.is_empty())
                .unwrap_or(&user)
                .to_string();
            Some((s["id"].as_str()?.to_string(), user, name))
        })
        .collect())
}

/// Full-text search over session titles and message content: (session id, snippet).
pub fn search_sessions(conn: &Connection, profile: &str, q: &str) -> Result<Vec<(String, String)>> {
    let v = get(
        conn,
        &format!("/api/sessions/search?q={}&limit=30", enc(q)),
        profile,
    )?;
    Ok(v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let snippet = r["snippet"].as_str().unwrap_or_default();
            let snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
            Some((r["session_id"].as_str()?.to_string(), snippet))
        })
        .collect())
}

/// The profile's defaults from config.yaml: (`agent.reasoning_effort`, model name).
/// (effort, primary model, [(model, provider)] primary first then `fallback_providers`).
/// (effort, primary model, models, approvals.mode).
pub type ProfileDefaults = (
    Option<String>,
    Option<String>,
    Vec<(String, String)>,
    String,
);

pub fn profile_defaults(conn: &Connection, profile: &str) -> Result<ProfileDefaults> {
    let raw = get(conn, "/api/config/raw", profile)?;
    let yaml: Value =
        serde_yaml::from_str(raw["yaml"].as_str().unwrap_or_default()).unwrap_or(Value::Null);
    let effort = yaml["agent"]["reasoning_effort"]
        .as_str()
        .map(str::to_lowercase);
    let model = match &yaml["model"] {
        Value::String(s) => Some(s.clone()),
        m => m["default"]
            .as_str()
            .or(m["name"].as_str())
            .map(str::to_string),
    }
    .filter(|m| !m.is_empty());
    let mut models: Vec<(String, String)> = vec![];
    if let Some(m) = &model {
        models.push((
            m.clone(),
            yaml["model"]["provider"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        ));
    }
    for f in yaml["fallback_providers"].as_array().into_iter().flatten() {
        if let Some(m) = f["model"].as_str() {
            let pair = (
                m.to_string(),
                f["provider"].as_str().unwrap_or_default().to_string(),
            );
            if !models.contains(&pair) {
                models.push(pair);
            }
        }
    }
    // Hermes' default is manual; a YAML 1.1 writer may have stored `off` as false.
    let approval = match &yaml["approvals"]["mode"] {
        Value::String(s) => s.to_lowercase(),
        Value::Bool(false) => "off".into(),
        _ => "manual".into(),
    };
    Ok((effort, model, models, approval))
}

pub fn skill_content(conn: &Connection, profile: &str, name: &str) -> Result<String> {
    let v = get(conn, &format!("/api/skills/content?name={name}"), profile)?;
    Ok(v["content"].as_str().unwrap_or_default().to_string())
}

/// The profile's `mcp_servers` map as pretty JSON, read from the raw config
/// (the summary endpoint masks secrets, and saving those masks would destroy them).
pub fn mcp_json(conn: &Connection, profile: &str) -> Result<String> {
    let raw = get(conn, "/api/config/raw", profile)?;
    let yaml: Value = serde_yaml::from_str(raw["yaml"].as_str().unwrap_or_default())
        .context("parse config.yaml")?;
    let servers = yaml.get("mcp_servers").cloned().unwrap_or(json!({}));
    Ok(serde_json::to_string_pretty(&servers)?)
}

pub fn save_mcp_json(conn: &Connection, profile: &str, text: &str) -> Result<()> {
    let servers: Value = serde_json::from_str(text).context("JSON")?;
    if !servers.is_object() {
        return Err(anyhow!("expected an object {{ \"name\": {{…}} }}"));
    }
    send(
        conn,
        "PUT",
        "/api/mcp/servers",
        profile,
        json!({"servers": servers}),
        30,
    )?;
    Ok(())
}

fn data_url_bytes(url: &str) -> Result<Vec<u8>> {
    let b64 = url
        .split_once(',')
        .map(|(_, b)| b)
        .ok_or_else(|| anyhow!("bad data url"))?;
    Ok(base64::engine::general_purpose::STANDARD.decode(b64)?)
}

/// Text -> audio bytes (mp3 or wav, as the profile's TTS provider returns).
pub fn speak(conn: &Connection, profile: &str, text: &str) -> Result<Vec<u8>> {
    let v = send(
        conn,
        "POST",
        "/api/audio/speak",
        profile,
        json!({"text": text}),
        180,
    )?;
    data_url_bytes(v["data_url"].as_str().unwrap_or_default())
}

pub fn transcribe(conn: &Connection, profile: &str, wav: &[u8]) -> Result<String> {
    let data_url = format!(
        "data:audio/wav;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(wav)
    );
    let v = send(
        conn,
        "POST",
        "/api/audio/transcribe",
        profile,
        json!({"data_url": data_url, "mime_type": "audio/wav"}),
        180,
    )?;
    Ok(v["transcript"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// Delete a stored session the way Hermes Desktop does: read its mutation snapshot, then
/// DELETE fenced on that revision (the gateway refuses an unfenced edit). Older gateways
/// without the snapshot route take a plain DELETE.
pub fn delete_session(conn: &Connection, profile: &str, id: &str) -> Result<()> {
    let path = format!("/api/sessions/{id}");
    let fence = match get(conn, &format!("{path}/mutation-snapshot"), profile) {
        Ok(snap) => format!(
            "request_id={}&expected_revision={}&expected_generation={}",
            uuid::Uuid::new_v4(),
            snap["runtime_revision"].as_i64().unwrap_or(0),
            snap["runtime_generation"].as_i64().unwrap_or(0)
        ),
        Err(e) if e.to_string().contains("404") || e.to_string().contains("405") => String::new(),
        Err(e) => return Err(e),
    };
    let mut target = url(conn, &path, profile);
    if !fence.is_empty() {
        target.push(if target.contains('?') { '&' } else { '?' });
        target.push_str(&fence);
    }
    let (h, v) = conn.rest_auth(profile)?;
    match ureq::request("DELETE", &target)
        .set(h, &v)
        .timeout(Duration::from_secs(30))
        .call()
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, r)) => {
            let body: Value = r.into_json().unwrap_or(Value::Null);
            Err(anyhow!(
                "{code}: {}",
                body["detail"].as_str().unwrap_or("delete failed")
            ))
        }
        Err(e) => Err(anyhow!("{e}")),
    }
}

/// Delete a skill folder (there is no skill-delete API; the managed files route removes it).
pub fn delete_skill(conn: &Connection, profile: &str, name: &str) -> Result<()> {
    let found = get(conn, &format!("/api/skills/content?name={name}"), profile)?;
    let skill_md = found["path"]
        .as_str()
        .ok_or_else(|| anyhow!("skill path unknown"))?;
    let dir = std::path::Path::new(skill_md)
        .parent()
        .ok_or_else(|| anyhow!("bad skill path"))?;
    send(
        conn,
        "DELETE",
        "/api/files",
        profile,
        json!({"path": dir.to_string_lossy(), "recursive": true}),
        30,
    )?;
    Ok(())
}

pub fn save_skill(conn: &Connection, profile: &str, name: &str, content: &str) -> Result<()> {
    send(
        conn,
        "PUT",
        "/api/skills/content",
        profile,
        json!({"name": name, "content": content}),
        30,
    )?;
    Ok(())
}

/// The profile's config.yaml as text.
pub fn config_raw(conn: &Connection, profile: &str) -> Result<String> {
    let raw = get(conn, "/api/config/raw", profile)?;
    Ok(raw["yaml"].as_str().unwrap_or_default().to_string())
}

pub fn save_config_raw(conn: &Connection, profile: &str, text: &str) -> Result<()> {
    send(
        conn,
        "PUT",
        "/api/config/raw",
        profile,
        json!({"yaml_text": text}),
        30,
    )?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct CronJob {
    pub id: String,
    /// The schedule as typed ("30 10 * * 1-5", "every 2h"), for editing.
    pub expr: String,
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
    pub next_run: String,
    pub last_status: String,
    pub prompt: String,
}

/// Scheduled jobs of this profile (read-only view).
pub fn cron_jobs(conn: &Connection, profile: &str) -> Result<Vec<CronJob>> {
    let key = if profile.is_empty() {
        "default"
    } else {
        profile
    };
    let (h, v) = conn.rest_auth(profile)?;
    let resp = ureq::get(&format!("{}/api/cron/jobs?profile={key}", conn.base))
        .set(h, &v)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| anyhow!("cron: {e}"))?;
    let v: Value = resp.into_json()?;
    let list = v
        .as_array()
        .cloned()
        .or_else(|| v["jobs"].as_array().cloned())
        .unwrap_or_default();
    let s = |j: &Value, keys: &[&str]| {
        keys.iter()
            .find_map(|k| match &j[*k] {
                Value::String(s) if !s.is_empty() => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Object(o) => o
                    .get("display")
                    .or(o.get("expr"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ => None,
            })
            .unwrap_or_default()
    };
    Ok(list
        .iter()
        .map(|j| CronJob {
            id: s(j, &["id", "job_id"]),
            expr: j["schedule"]["expr"]
                .as_str()
                .or(j["schedule"].as_str())
                .map(str::to_string)
                .unwrap_or_else(|| s(j, &["schedule_display"])),
            name: s(j, &["name", "id", "job_id"]),
            schedule: s(j, &["schedule_display", "schedule", "cron", "interval"]),
            enabled: j["enabled"].as_bool().unwrap_or(j["state"] != "paused"),
            next_run: s(j, &["next_run_at", "next_run"]),
            last_status: s(j, &["last_status", "last_result"]),
            prompt: s(j, &["prompt", "task", "message"]),
        })
        .collect())
}

/// A text file as the gateway sees it. `truncated` files must not be saved back.
pub struct TextFile {
    pub text: String,
    pub truncated: bool,
    pub binary: bool,
}

pub fn read_text(conn: &Connection, profile: &str, path: &str) -> Result<TextFile> {
    let v = get(
        conn,
        &format!("/api/fs/read-text?path={}", enc(path)),
        profile,
    )?;
    Ok(TextFile {
        text: v["text"].as_str().unwrap_or_default().to_string(),
        truncated: v["truncated"].as_bool().unwrap_or(false),
        binary: v["binary"].as_bool().unwrap_or(false),
    })
}

pub fn write_text(conn: &Connection, profile: &str, path: &str, content: &str) -> Result<()> {
    send(
        conn,
        "POST",
        "/api/fs/write-text",
        profile,
        json!({"path": path, "content": content}),
        30,
    )?;
    Ok(())
}

/// Directory entries: (name, full path, is_dir), directories first.
pub fn list_dir(
    conn: &Connection,
    profile: &str,
    path: &str,
) -> Result<Vec<(String, String, bool)>> {
    let v = get(conn, &format!("/api/fs/list?path={}", enc(path)), profile)?;
    if let Some(err) = v["error"].as_str() {
        return Err(anyhow!("{err}"));
    }
    Ok(v["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            (
                e["name"].as_str().unwrap_or_default().to_string(),
                e["path"].as_str().unwrap_or_default().to_string(),
                e["isDirectory"].as_bool().unwrap_or(false),
            )
        })
        .collect())
}

pub fn default_cwd(conn: &Connection, profile: &str) -> Result<String> {
    let v = get(conn, "/api/fs/default-cwd", profile)?;
    Ok(v["cwd"].as_str().unwrap_or_default().to_string())
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Raw GET, for diagnostics (`--probe`).
pub fn get_raw(conn: &Connection, profile: &str, path: &str) -> Result<Value> {
    get(conn, path, profile)
}

/// Each session's folder and git info, by session id (the most recent 100, the API maximum).
pub fn session_places(
    conn: &Connection,
    profile: &str,
) -> Result<std::collections::HashMap<String, crate::model::Place>> {
    let v = get(conn, "/api/sessions?limit=100&order=recent", profile)?;
    let s = |r: &Value, k: &str| r[k].as_str().unwrap_or_default().to_string();
    Ok(v["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let place = crate::model::Place {
                cwd: s(r, "cwd"),
                repo: s(r, "git_repo_root"),
                branch: s(r, "git_branch"),
            };
            Some((r["id"].as_str()?.to_string(), place))
        })
        .collect())
}

/// A new branch in its own folder (`<repo>/.worktrees/<name>`): (folder, branch).
pub fn worktree_add(
    conn: &Connection,
    profile: &str,
    repo: &str,
    name: &str,
) -> Result<(String, String)> {
    let v = send(
        conn,
        "POST",
        "/api/git/worktree/add",
        profile,
        json!({"path": repo, "name": name}),
        60,
    )?;
    Ok((
        v["path"]
            .as_str()
            .ok_or_else(|| anyhow!("no worktree path"))?
            .to_string(),
        v["branch"].as_str().unwrap_or_default().to_string(),
    ))
}

pub fn worktree_remove(conn: &Connection, profile: &str, repo: &str, folder: &str) -> Result<()> {
    send(
        conn,
        "POST",
        "/api/git/worktree/remove",
        profile,
        json!({"path": repo, "worktreePath": folder, "force": true}),
        60,
    )?;
    Ok(())
}

fn cron_profile(profile: &str) -> &str {
    if profile.is_empty() {
        "default"
    } else {
        profile
    }
}

/// Save a job's text and schedule (the gateway re-derives its next run).
pub fn update_cron(
    conn: &Connection,
    profile: &str,
    id: &str,
    prompt: &str,
    schedule: &str,
) -> Result<()> {
    send(
        conn,
        "PUT",
        &format!(
            "/api/cron/jobs/{}?profile={}",
            enc(id),
            enc(cron_profile(profile))
        ),
        profile,
        json!({"updates": {"prompt": prompt, "schedule": schedule}}),
        30,
    )?;
    Ok(())
}

/// "trigger" (run now), "pause" or "resume".
pub fn cron_action(conn: &Connection, profile: &str, id: &str, action: &str) -> Result<()> {
    send(
        conn,
        "POST",
        &format!(
            "/api/cron/jobs/{}/{action}?profile={}",
            enc(id),
            enc(cron_profile(profile))
        ),
        profile,
        json!({}),
        30,
    )?;
    Ok(())
}
