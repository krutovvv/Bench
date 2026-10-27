//! The agent's model providers, keys and messaging: custom endpoints, environment keys, the
//! config, the gateway's status.

use super::{enc, get, send};
use crate::client::connection::Connection;
use anyhow::Result;
use serde_json::{Value, json};

// ---- providers, keys, messaging ---------------------------------------------------------

/// The profile's custom (OpenAI-compatible) endpoints: `{endpoints: [{id, name, base_url,
/// model, models, context_length, has_api_key, api_key_preview, is_current}], current}`.
pub fn custom_endpoints(conn: &Connection, profile: &str) -> Result<Value> {
    get(conn, "/api/providers/custom-endpoints", profile)
}

/// Create or update an endpoint. `api_key: None` keeps the saved key.
pub fn save_endpoint(conn: &Connection, profile: &str, body: Value) -> Result<Value> {
    send(
        conn,
        "POST",
        "/api/providers/custom-endpoints",
        profile,
        body,
        30,
    )
}

/// The endpoint becomes the profile's main model provider.
pub fn activate_endpoint(conn: &Connection, profile: &str, id: &str) -> Result<()> {
    send(
        conn,
        "POST",
        &format!("/api/providers/custom-endpoints/{}/activate", enc(id)),
        profile,
        json!({}),
        30,
    )?;
    Ok(())
}

pub fn delete_endpoint(conn: &Connection, profile: &str, id: &str) -> Result<()> {
    send(
        conn,
        "DELETE",
        &format!("/api/providers/custom-endpoints/{}", enc(id)),
        profile,
        json!({}),
        30,
    )?;
    Ok(())
}

/// Probe an endpoint's `/models`: `{ok, message, models}`.
pub fn validate_endpoint(conn: &Connection, profile: &str, body: Value) -> Result<Value> {
    send(
        conn,
        "POST",
        "/api/providers/custom-endpoints/validate",
        profile,
        body,
        30,
    )
}

/// The profile's `.env` keys the agent knows or holds: name → `{is_set, redacted_value,
/// description, category, channel_managed, custom}`.
pub fn env_vars(conn: &Connection, profile: &str) -> Result<Value> {
    get(conn, "/api/env", profile)
}

/// Merge keys into the profile's config (the agent deep-merges what is sent over it).
pub fn merge_config(conn: &Connection, profile: &str, config: Value) -> Result<()> {
    send(
        conn,
        "PUT",
        "/api/config",
        profile,
        json!({"config": config}),
        30,
    )?;
    Ok(())
}

/// The gateway's own state (`/api/status`): what the status bar's backend item shows.
#[derive(Clone, Debug, Default)]
pub struct GatewayStatus {
    pub version: String,
    pub running: bool,
    /// "running", "starting", "draining", "stopped"…
    pub state: String,
    /// "ok" or "degraded".
    pub overall: String,
    pub active_agents: u64,
    /// (name as the agent keys it, state, error message)
    pub platforms: Vec<(String, String, Option<String>)>,
    pub exit_reason: Option<String>,
}

pub fn gateway_status(conn: &Connection, profile: &str) -> Result<GatewayStatus> {
    let v = get(conn, "/api/status", profile)?;
    let mut platforms: Vec<(String, String, Option<String>)> = v["gateway_platforms"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, p)| {
            (
                name.clone(),
                p["state"].as_str().unwrap_or("unknown").to_string(),
                p["error_message"].as_str().map(str::to_string),
            )
        })
        .collect();
    platforms.sort();
    Ok(GatewayStatus {
        version: v["version"].as_str().unwrap_or_default().to_string(),
        running: v["gateway_running"].as_bool().unwrap_or(false),
        state: v["gateway_state"].as_str().unwrap_or_default().to_string(),
        overall: v["overall"].as_str().unwrap_or("ok").to_string(),
        active_agents: v["active_agents"].as_u64().unwrap_or(0),
        platforms,
        exit_reason: v["gateway_exit_reason"].as_str().map(str::to_string),
    })
}
