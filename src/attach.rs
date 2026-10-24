//! Files sent with a message. Images (png, jpeg, gif, webp) go to the model as real attachments
//! (staged in the profile's image cache, which `prompt.submit` requires); every file is also
//! named in the message as `[attached: <path>]`, a path on the gateway's machine the agent can
//! read. A local file is used in place unless it lives in a temporary folder (a dragged
//! screenshot), which is copied to the profile's `attachments/`; over SSH files are uploaded.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::connection::{Connection, state_dir};
use crate::model::Target;
use crate::rest;

/// What a message carries for its files.
#[derive(Default)]
pub struct Prepared {
    /// `[attached: …]` lines for the text.
    pub lines: Vec<String>,
}

pub const PREFIX: &str = "[attached: ";

/// The paths named by `[attached: …]` lines, and the text without them.
pub fn split(text: &str) -> (String, Vec<String>) {
    let mut paths = vec![];
    let mut images = vec![];
    let mut rest = vec![];
    for line in text.lines() {
        let t = line.trim();
        if let Some(p) = t.strip_prefix(PREFIX).and_then(|p| p.strip_suffix(']')) {
            paths.push(p.to_string());
        } else if let Some(p) = t
            .strip_prefix(GATEWAY_IMAGE)
            .and_then(|p| p.strip_suffix(']'))
        {
            images.push(p.to_string());
        } else {
            rest.push(line);
        }
    }
    // Our own lines already name every file; the gateway's notes only stand in without them.
    if paths.is_empty() {
        paths = images;
    }
    (rest.join("\n").trim().to_string(), paths)
}

/// The gateway's own note for an image attachment, stored in the message.
const GATEWAY_IMAGE: &str = "[Image attached at: ";

fn is_temporary(path: &Path) -> bool {
    let p = path.to_string_lossy();
    [
        "/var/folders/",
        "/private/var/folders/",
        "/tmp/",
        "/private/tmp/",
    ]
    .iter()
    .any(|t| p.starts_with(t))
        || p.starts_with(&state_dir().join("pasted").to_string_lossy().to_string())
}

/// Blocking: call off the UI thread.
pub fn prepare(
    conn: &Connection,
    target: &Target,
    profile: &str,
    files: &[PathBuf],
) -> Result<Prepared> {
    let mut out = Prepared::default();
    let mut home: Option<String> = None;
    let stamp = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    for file in files {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into());
        let bytes = std::fs::read(file).with_context(|| format!("read {name}"))?;
        let path = if *target == Target::Local && !is_temporary(file) {
            file.to_string_lossy().to_string()
        } else {
            if home.is_none() {
                home = Some(rest::profile_home(conn, profile)?);
            }
            let dest = format!(
                "{}/attachments/{stamp}-{name}",
                home.as_deref().unwrap_or_default()
            );
            match target {
                Target::Local => {
                    std::fs::create_dir_all(Path::new(&dest).parent().unwrap())?;
                    std::fs::write(&dest, &bytes)?;
                }
                Target::Ssh(_) => rest::upload_file(conn, profile, &dest, &bytes)?,
            }
            dest
        };
        out.lines.push(format!("{PREFIX}{path}]"));
    }
    Ok(out)
}

/// A pasted image (no file of its own) saved where it can be attached from.
pub fn save_pasted(bytes: &[u8], ext: &str) -> Result<PathBuf> {
    let dir = state_dir().join("pasted");
    std::fs::create_dir_all(&dir)?;
    let stamp = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let path = dir.join(format!("Screenshot-{stamp}.{ext}"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}
