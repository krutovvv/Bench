//! Blocking REST calls to the gateway's dashboard API. Call them off the UI thread.

mod files;
mod providers;

pub use files::*;
pub use providers::*;

use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use base64::Engine as _;
use serde_json::{Value, json};

use crate::client::connection::Connection;
use crate::app::model::SkillRow;

fn url(conn: &Connection, path: &str, profile: &str) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    if profile.is_empty() || profile == "default" {
        format!("{}{path}", conn.base)
    } else {
        format!("{}{path}{sep}profile={profile}", conn.base)
    }
}

/// The purpose a route's auth names (from the ticket protocol, Hermes Desktop's
/// `nativeGrantPurpose`); with the agent's session token it no longer matters.
fn purpose(method: &str, path: &str) -> &'static str {
    let path = path
        .split('?')
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    let named = path
        .strip_prefix("/api/profiles/")
        .is_some_and(|s| !s.is_empty() && !s.contains('/') && !matches!(s, "active" | "import"));
    match method {
        "POST" if path == "/api/profiles" => "native-profile-create",
        "PATCH" | "DELETE" if named => "native-profile-registry",
        _ => "native-http",
    }
}

/// A GET answered with JSON, for what is no one profile's (the agent's status).
pub fn get_json(conn: &Connection, path: &str) -> Result<Value> {
    get(conn, path, "")
}

fn get(conn: &Connection, path: &str, profile: &str) -> Result<Value> {
    let (h, v) = conn.rest_auth(profile, "native-http")?;
    let resp = ureq::get(&url(conn, path, profile))
        .set(h, &v)
        .timeout(Duration::from_secs(30))
        .call();
    match resp {
        Ok(r) => Ok(r.into_json()?),
        // The agent's own message ("501 Login incorrect."), not the URL.
        Err(ureq::Error::Status(code, r)) => {
            let body: Value = r.into_json().unwrap_or(Value::Null);
            Err(anyhow!(
                "{}",
                body["detail"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{path}: status {code}"))
            ))
        }
        Err(e) => Err(anyhow!("{path}: {e}")),
    }
}

/// A JSON request (POST, PUT, DELETE) with a 30 s timeout.
pub fn send_json(
    conn: &Connection,
    method: &str,
    path: &str,
    profile: &str,
    body: Value,
) -> Result<Value> {
    send(conn, method, path, profile, body, 30)
}

/// A JSON request that waits `secs`. Asking the MCP servers to answer is the one job that can
/// legitimately take longer than 30 s: the agent gives each server 30 s to shake hands, then 30 s
/// to list its tools, so the answer must not be cut off before his.
pub fn send_json_secs(
    conn: &Connection,
    method: &str,
    path: &str,
    profile: &str,
    body: Value,
    secs: u64,
) -> Result<Value> {
    send(conn, method, path, profile, body, secs)
}

fn send(
    conn: &Connection,
    method: &str,
    path: &str,
    profile: &str,
    body: Value,
    timeout: u64,
) -> Result<Value> {
    let (h, v) = conn.rest_auth(profile, purpose(method, path))?;
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

/// One model the picker offers: which provider serves it, the group it is listed under, and
/// whether it is the profile's main model or a fallback.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelChoice {
    pub model: String,
    pub provider: String,
    pub group: String,
    pub tag: &'static str,
}

/// The profile's defaults from config.yaml: (`agent.reasoning_effort`, primary model, the
/// models it offers — main first, then fallbacks, then the rest of each provider's list —
/// `approvals.mode`, `model.context_length` or 0).
pub type ProfileDefaults = (
    Option<String>,
    Option<String>,
    Vec<ModelChoice>,
    String,
    u64,
);

/// A provider id as people read it.
pub fn provider_group(provider: &str, yaml: &Value) -> String {
    match provider {
        "openrouter" => "OpenRouter".into(),
        "" => "Model".into(),
        p => yaml["providers"][p]["name"]
            .as_str()
            .unwrap_or(p.strip_prefix("custom:").unwrap_or(p))
            .to_string(),
    }
}

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
    let mut models: Vec<ModelChoice> = vec![];
    let add = |models: &mut Vec<ModelChoice>, m: &str, p: &str, tag: &'static str| {
        if m.is_empty() || models.iter().any(|c| c.model == m && c.provider == p) {
            return;
        }
        models.push(ModelChoice {
            model: m.to_string(),
            provider: p.to_string(),
            group: provider_group(p, &yaml),
            tag,
        });
    };
    if let Some(m) = &model {
        let p = yaml["model"]["provider"].as_str().unwrap_or_default();
        add(&mut models, m, p, "main");
    }
    for f in yaml["fallback_providers"].as_array().into_iter().flatten() {
        if let Some(m) = f["model"].as_str() {
            add(
                &mut models,
                m,
                f["provider"].as_str().unwrap_or_default(),
                "fallback",
            );
        }
    }
    // Every custom endpoint's models (`providers.<id>.models` keys, or its one model).
    if let Some(map) = yaml["providers"].as_object() {
        for (id, entry) in map {
            if entry["base_url"]
                .as_str()
                .or(entry["api"].as_str())
                .is_none()
            {
                continue;
            }
            let listed: Vec<String> = match &entry["models"] {
                Value::Object(m) => m.keys().cloned().collect(),
                Value::Array(a) => a
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
                _ => vec![],
            };
            let one = entry["model"].as_str().or(entry["default_model"].as_str());
            for m in listed.iter().map(String::as_str).chain(one) {
                add(&mut models, m, id, "");
            }
        }
    }
    // The default is manual; a YAML 1.1 writer may have stored `off` as false.
    let approval = match &yaml["approvals"]["mode"] {
        Value::String(s) => s.to_lowercase(),
        Value::Bool(false) => "off".into(),
        _ => "manual".into(),
    };
    let context = yaml["model"]["context_length"].as_u64().unwrap_or(0);
    Ok((effort, model, models, approval, context))
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

/// Delete a stored session: read its mutation snapshot, then DELETE fenced on that revision;
/// a gateway without the snapshot route (the agent) takes a plain DELETE.
pub fn delete_session(conn: &Connection, profile: &str, id: &str) -> Result<()> {
    let target = url(conn, &format!("/api/sessions/{id}"), profile);
    let (h, v) = conn.rest_auth(profile, "native-http")?;
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

/// The profile's config.yaml as text.
pub fn config_raw(conn: &Connection, profile: &str) -> Result<String> {
    let raw = get(conn, "/api/config/raw", profile)?;
    Ok(raw["yaml"].as_str().unwrap_or_default().to_string())
}

#[derive(Clone, Debug)]
pub struct CronJob {
    pub id: String,
    /// The schedule as typed ("30 10 * * 1-5", "every 2h"), for editing.
    pub expr: String,
    pub name: String,
    pub schedule: String,
    pub next_run: String,
}

/// Scheduled jobs of this profile (read-only view).
pub fn cron_jobs(conn: &Connection, profile: &str) -> Result<Vec<CronJob>> {
    let key = if profile.is_empty() {
        "default"
    } else {
        profile
    };
    let (h, v) = conn.rest_auth(profile, "native-http")?;
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
            next_run: s(j, &["next_run_at", "next_run"]),
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
) -> Result<std::collections::HashMap<String, crate::app::model::Place>> {
    let v = get(conn, "/api/sessions?limit=100&order=recent", profile)?;
    let s = |r: &Value, k: &str| r[k].as_str().unwrap_or_default().to_string();
    Ok(v["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let place = crate::app::model::Place {
                cwd: s(r, "cwd"),
                repo: s(r, "git_repo_root"),
                branch: s(r, "git_branch"),
            };
            Some((r["id"].as_str()?.to_string(), place))
        })
        .collect())
}

/// The profile's folder in the agent's home on its machine: `~/.bench/profiles/<name>`
/// (`default` for the default profile).
pub fn profile_home(conn: &Connection, profile: &str) -> Result<String> {
    let v = get(conn, "/api/chat/workspaces", profile)?;
    let home = v["home"]
        .as_str()
        .ok_or_else(|| anyhow!("home folder unknown"))?;
    let name = if profile.is_empty() {
        "default"
    } else {
        profile
    };
    Ok(format!("{home}/.bench/profiles/{name}"))
}

