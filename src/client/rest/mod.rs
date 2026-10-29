//! Blocking REST calls to the agent (`/api/…`): files, settings, providers, projects. Call them
//! off the UI thread. `agent` names the agent settings a call is about: "default", or a
//! project's `project:<id>`.

mod files;
pub mod lsp;
mod providers;

pub use files::*;
pub use providers::*;

use std::time::Duration;

use anyhow::{Result, anyhow};
use base64::Engine as _;
use serde_json::{Value, json};

use crate::client::connection::Connection;

fn url(conn: &Connection, path: &str) -> String {
    format!("{}{path}", conn.base)
}

/// A route about agent settings, for the settings `settings` names: "default" (or "") the agent's
/// general ones, `project:<id>` a project's (`env::settings_of`).
pub fn with_settings(path: &str, settings: &str) -> String {
    if settings.is_empty() || settings == crate::app::model::GENERAL_AGENT {
        return path.to_string();
    }
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{path}{sep}agent={settings}")
}

/// A GET answered with JSON.
pub fn get(conn: &Connection, path: &str) -> Result<Value> {
    let (h, v) = conn.auth();
    let resp = ureq::get(&url(conn, path))
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
    body: Value,
) -> Result<Value> {
    send(conn, method, path, body, 30)
}

/// A JSON request that waits `secs`. Asking the MCP servers to answer is the one job that can
/// legitimately take longer than 30 s: the agent gives each server 30 s to shake hands, then 30 s
/// to list its tools, so the answer must not be cut off before his.
pub fn send_json_secs(
    conn: &Connection,
    method: &str,
    path: &str,
    body: Value,
    secs: u64,
) -> Result<Value> {
    send(conn, method, path, body, secs)
}

fn send(
    conn: &Connection,
    method: &str,
    path: &str,
    body: Value,
    timeout: u64,
) -> Result<Value> {
    let (h, v) = conn.auth();
    let resp = ureq::request(method, &url(conn, path))
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

/// Full-text search over thread titles and message content: (thread id, snippet).
pub fn search_threads(conn: &Connection, q: &str) -> Result<Vec<(String, String)>> {
    let v = get(
        conn,
        &format!("/api/threads/search?q={}&limit=30", enc(q)),
    )?;
    Ok(v["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let snippet = r["snippet"].as_str().unwrap_or_default();
            let snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
            Some((r["thread"].as_str()?.to_string(), snippet))
        })
        .collect())
}

/// One model the picker offers: which provider serves it, the group it is listed under, and
/// whether it is the agent's main model or a fallback.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelChoice {
    pub model: String,
    pub provider: String,
    pub group: String,
    pub tag: &'static str,
}

/// The agent's defaults from config.yaml: (`agent.reasoning_effort`, primary model, the
/// models it offers — main first, then fallbacks, then the rest of each provider's list —
/// `approvals.mode`, `model.context_length` or 0).
pub type AgentDefaults = (
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

pub fn agent_defaults(conn: &Connection, settings: &str) -> Result<AgentDefaults> {
    let raw = get(conn, &with_settings("/api/config/raw", settings))?;
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

fn data_url_bytes(url: &str) -> Result<Vec<u8>> {
    let b64 = url
        .split_once(',')
        .map(|(_, b)| b)
        .ok_or_else(|| anyhow!("bad data url"))?;
    Ok(base64::engine::general_purpose::STANDARD.decode(b64)?)
}

/// Text -> audio bytes (mp3 or wav, as the agent's TTS provider returns).
pub fn speak(conn: &Connection, text: &str) -> Result<Vec<u8>> {
    let v = send(
        conn,
        "POST",
        "/api/audio/speak",
        json!({"text": text}),
        180,
    )?;
    data_url_bytes(v["data_url"].as_str().unwrap_or_default())
}

/// `prompt`: the text said before this audio, a hint for the recognizer (dictation in pieces).
pub fn transcribe(conn: &Connection, wav: &[u8], prompt: &str) -> Result<String> {
    let data_url = format!(
        "data:audio/wav;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(wav)
    );
    let v = send(
        conn,
        "POST",
        "/api/audio/transcribe",
        json!({"data_url": data_url, "mime_type": "audio/wav", "prompt": prompt}),
        180,
    )?;
    Ok(v["transcript"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// The agent's config.yaml as text.
pub fn config_raw(conn: &Connection) -> Result<String> {
    let raw = get(conn, "/api/config/raw")?;
    Ok(raw["yaml"].as_str().unwrap_or_default().to_string())
}

pub fn default_cwd(conn: &Connection) -> Result<String> {
    let v = get(conn, "/api/fs/default-cwd")?;
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

/// Where attachments go on the agent's machine: `~/.bench/attachments` in its home.
pub fn attachments_dir(conn: &Connection) -> Result<String> {
    let v = get(conn, "/api/chat/workspaces")?;
    let home = v["home"]
        .as_str()
        .ok_or_else(|| anyhow!("home folder unknown"))?;
    Ok(format!("{home}/.bench/attachments"))
}
