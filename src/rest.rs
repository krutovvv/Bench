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

/// The profile's defaults from config.yaml: (`agent.reasoning_effort`, model name).
pub fn profile_defaults(
    conn: &Connection,
    profile: &str,
) -> Result<(Option<String>, Option<String>)> {
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
    Ok((effort, model))
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
        return Err(anyhow!("ожидается объект {{ \"имя\": {{…}} }}"));
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
