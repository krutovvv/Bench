//! Files through the agent: read, write, list and download on the agent's machine or a project's
//! (`machine`), and uploads.

use super::{enc, get, send, url};
use crate::client::connection::Connection;
use anyhow::{Result, anyhow};
use base64::Engine as _;
use serde_json::{Value, json};
use std::time::Duration;

/// A text file as the agent sees it. `truncated` files must not be saved back.
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
    machine: Option<&str>,
    path: &str,
) -> Result<TextFile> {
    let v = get(
        conn,
        &format!("/api/fs/read-text?path={}{}", enc(path), on(machine)),
    )?;
    Ok(TextFile {
        text: v["text"].as_str().unwrap_or_default().to_string(),
        truncated: v["truncated"].as_bool().unwrap_or(false),
        binary: v["binary"].as_bool().unwrap_or(false),
    })
}

pub fn write_text(
    conn: &Connection,
    machine: Option<&str>,
    path: &str,
    content: &str,
) -> Result<()> {
    send(
        conn,
        "POST",
        "/api/fs/write-text",
        json!({"path": path, "content": content, "machine": machine}),
        30,
    )?;
    Ok(())
}

/// Directory entries: (name, full path, is_dir), directories first.
pub fn list_dir(
    conn: &Connection,
    machine: Option<&str>,
    path: &str,
) -> Result<Vec<(String, String, bool)>> {
    let v = get(
        conn,
        &format!("/api/fs/list?path={}{}", enc(path), on(machine)),
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

/// A file's bytes as the agent's machine has them (at most `cap`).
pub fn download(
    conn: &Connection,
    machine: Option<&str>,
    path: &str,
    cap: u64,
) -> Result<Vec<u8>> {
    use std::io::Read as _;
    if conn.fake.is_some() {
        return Err(anyhow!("a fake agent keeps no files"));
    }
    let route = format!("/api/fs/download?path={}{}", enc(path), on(machine));
    let (h, v) = conn.auth();
    let resp = match ureq::get(&url(conn, &route))
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

/// Write a file on the agent's machine (inside the agent's managed files).
pub fn upload_file(conn: &Connection, path: &str, bytes: &[u8]) -> Result<()> {
    upload_file_to(conn, None, path, bytes)
}

/// Write a file on `machine` (None: the agent's own) through the agent.
pub fn upload_file_to(
    conn: &Connection,
    machine: Option<&str>,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    if machine.is_some() {
        send(
            conn,
            "POST",
            "/api/files/upload",
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
        json!({"path": dir}),
        30,
    );
    send(
        conn,
        "POST",
        "/api/files/upload",
        json!({"path": path, "data_url": data_url("application/octet-stream", bytes), "overwrite": true}),
        600,
    )?;
    Ok(())
}

/// A file's text at HEAD of its repository (`/api/git/head`); None: not in one, or not committed.
pub fn git_head(
    conn: &Connection,
    machine: Option<&str>,
    path: &str,
) -> Result<Option<String>> {
    let v = get(
        conn,
        &format!("/api/git/head?path={}{}", enc(path), on(machine)),
    )?;
    Ok(v["text"].as_str().map(str::to_string))
}

/// The specs of the repository around `path` (`/api/specs`, `agent/src/specs.ts`), as JSON.
pub fn specs(conn: &Connection, machine: Option<&str>, path: &str) -> Result<Value> {
    get(conn, &format!("/api/specs?path={}{}", enc(path), on(machine)))
}

/// A file as a git tree has it (a turn's start or end, `agent/src/runtime/changes.ts`): its text,
/// or None when the tree has no such file.
pub fn git_show(
    conn: &Connection,
    machine: Option<&str>,
    root: &str,
    tree: &str,
    path: &str,
) -> Result<Option<String>> {
    let v = get(
        conn,
        &format!(
            "/api/git/show?root={}&tree={}&path={}{}",
            enc(root),
            enc(tree),
            enc(path),
            on(machine)
        ),
    )?;
    Ok(v["text"].as_str().map(str::to_string))
}

/// The repository around `path` and what git ignores there (`/api/git/ignored`): the root, then
/// paths relative to it — a trailing '/' marks a folded folder. "": not a repository.
pub fn git_ignored(
    conn: &Connection,
    machine: Option<&str>,
    path: &str,
) -> Result<(String, Vec<String>)> {
    let v = get(
        conn,
        &format!("/api/git/ignored?path={}{}", enc(path), on(machine)),
    )?;
    let ignored = v["ignored"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Ok((v["root"].as_str().unwrap_or_default().to_string(), ignored))
}
