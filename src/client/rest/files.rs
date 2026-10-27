//! Files through the agent: read, write, list and download on the agent's machine or a project's
//! (`machine`), and uploads.

use std::time::Duration;
use anyhow::{Result, anyhow};
use base64::Engine as _;
use serde_json::{Value, json};
use crate::client::connection::Connection;
use super::{enc, get, send, url};

/// A text file as the gateway sees it. `truncated` files must not be saved back.
pub struct TextFile {
    pub text: String,
    pub truncated: bool,
    pub binary: bool,
}

/// `&machine=<alias>` for a file on the project's machine rather than the agent's own.
fn on(machine: Option<&str>) -> String {
    machine.map_or(String::new(), |m| format!("&machine={}", enc(m)))
}

pub fn read_text(
    conn: &Connection,
    profile: &str,
    machine: Option<&str>,
    path: &str,
) -> Result<TextFile> {
    let v = get(
        conn,
        &format!("/api/fs/read-text?path={}{}", enc(path), on(machine)),
        profile,
    )?;
    Ok(TextFile {
        text: v["text"].as_str().unwrap_or_default().to_string(),
        truncated: v["truncated"].as_bool().unwrap_or(false),
        binary: v["binary"].as_bool().unwrap_or(false),
    })
}

pub fn write_text(
    conn: &Connection,
    profile: &str,
    machine: Option<&str>,
    path: &str,
    content: &str,
) -> Result<()> {
    send(
        conn,
        "POST",
        "/api/fs/write-text",
        profile,
        json!({"path": path, "content": content, "machine": machine}),
        30,
    )?;
    Ok(())
}

/// Directory entries: (name, full path, is_dir), directories first.
pub fn list_dir(
    conn: &Connection,
    profile: &str,
    machine: Option<&str>,
    path: &str,
) -> Result<Vec<(String, String, bool)>> {
    let v = get(
        conn,
        &format!("/api/fs/list?path={}{}", enc(path), on(machine)),
        profile,
    )?;
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

/// A file's bytes as the gateway's machine has them (at most `cap`).
pub fn download(
    conn: &Connection,
    profile: &str,
    machine: Option<&str>,
    path: &str,
    cap: u64,
) -> Result<Vec<u8>> {
    use std::io::Read as _;
    let route = format!("/api/fs/download?path={}{}", enc(path), on(machine));
    let (h, v) = conn.rest_auth(profile, "native-http")?;
    let resp = match ureq::get(&url(conn, &route, profile))
        .set(h, &v)
        .timeout(Duration::from_secs(300))
        .call()
    {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            let body: Value = r.into_json().unwrap_or(Value::Null);
            return Err(anyhow!(
                "{code}: {}",
                body["detail"].as_str().unwrap_or("download failed")
            ));
        }
        Err(e) => return Err(anyhow!("{e}")),
    };
    let mut bytes = vec![];
    resp.into_reader().take(cap + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > cap {
        return Err(anyhow!("file is over {} MB", cap >> 20));
    }
    Ok(bytes)
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// Write a file on the gateway's machine (inside the profile's managed files).
pub fn upload_file(conn: &Connection, profile: &str, path: &str, bytes: &[u8]) -> Result<()> {
    upload_file_to(conn, profile, None, path, bytes)
}

/// Write a file on `machine` (None: the agent's own) through the agent.
pub fn upload_file_to(
    conn: &Connection,
    profile: &str,
    machine: Option<&str>,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    if machine.is_some() {
        send(
            conn,
            "POST",
            "/api/files/upload",
            profile,
            json!({"path": path, "data_url": data_url("application/octet-stream", bytes), "overwrite": true, "machine": machine}),
            600,
        )?;
        return Ok(());
    }
    let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or(path);
    let _ = send(
        conn,
        "POST",
        "/api/files/mkdir",
        profile,
        json!({"path": dir}),
        30,
    );
    send(
        conn,
        "POST",
        "/api/files/upload",
        profile,
        json!({"path": path, "data_url": data_url("application/octet-stream", bytes), "overwrite": true}),
        600,
    )?;
    Ok(())
}
