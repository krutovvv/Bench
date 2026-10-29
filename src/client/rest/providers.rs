//! The agent's model providers, keys and messaging: custom endpoints, environment keys, the
//! config, the agent's status.

use super::{enc, get, send};
use crate::client::connection::Connection;
use anyhow::Result;
use serde_json::{Value, json};

// ---- providers, keys, messaging ---------------------------------------------------------

/// The agent's custom (OpenAI-compatible) endpoints: `{endpoints: [{id, name, base_url,
/// model, models, context_length, has_api_key, api_key_preview, is_current}], current}`.
pub fn custom_endpoints(conn: &Connection) -> Result<Value> {
    get(conn, "/api/providers/custom-endpoints")
}

/// Create or update an endpoint. `api_key: None` keeps the saved key.
pub fn save_endpoint(conn: &Connection, body: Value) -> Result<Value> {
    send(
        conn,
        "POST",
        "/api/providers/custom-endpoints",
        body,
        30,
    )
}

/// The endpoint becomes the agent's main model provider.
pub fn activate_endpoint(conn: &Connection, id: &str) -> Result<()> {
    send(
        conn,
        "POST",
        &format!("/api/providers/custom-endpoints/{}/activate", enc(id)),
        json!({}),
        30,
    )?;
    Ok(())
}

pub fn delete_endpoint(conn: &Connection, id: &str) -> Result<()> {
    send(
        conn,
        "DELETE",
        &format!("/api/providers/custom-endpoints/{}", enc(id)),
        json!({}),
        30,
    )?;
    Ok(())
}

/// Probe an endpoint's `/models`: `{ok, message, models}`.
pub fn validate_endpoint(conn: &Connection, body: Value) -> Result<Value> {
    send(
        conn,
        "POST",
        "/api/providers/custom-endpoints/validate",
        body,
        30,
    )
}

/// Ask the evaluator (a System One server) one question with these settings: `{ok, message}`.
/// Long: a local server may start and load its model first.
pub fn check_judge(conn: &Connection, body: Value) -> Result<Value> {
    send(conn, "POST", "/api/judge/check", body, 170)
}

/// The evaluator's servers: `{servers: [{id, name, base_url, model, api_key}], main, fallback}`.
/// Each change below answers with the list as it is then.
pub fn judge(conn: &Connection) -> Result<Value> {
    get(conn, "/api/judge")
}

/// Add or change a server; no `api_key` keeps the saved one.
pub fn judge_save(conn: &Connection, body: Value) -> Result<Value> {
    send(conn, "PUT", "/api/judge/server", body, 30)
}

pub fn judge_delete(conn: &Connection, id: &str) -> Result<Value> {
    send(conn, "DELETE", "/api/judge/server", json!({"id": id}), 30)
}

pub fn judge_main(conn: &Connection, id: &str) -> Result<Value> {
    send(conn, "POST", "/api/judge/main", json!({"id": id}), 30)
}

/// The fallback server, or none.
pub fn judge_fallback(conn: &Connection, id: Option<&str>) -> Result<Value> {
    send(conn, "POST", "/api/judge/fallback", json!({"id": id}), 30)
}

/// The agent's `.env` keys the agent knows or holds: name → `{is_set, redacted_value,
/// description, category, channel_managed, custom}`.
pub fn env_vars(conn: &Connection) -> Result<Value> {
    get(conn, "/api/env")
}

/// Merge keys into the agent's config (the agent deep-merges what is sent over it).
pub fn merge_config(conn: &Connection, config: Value) -> Result<()> {
    send(
        conn,
        "PUT",
        "/api/config",
        json!({"config": config}),
        30,
    )?;
    Ok(())
}

