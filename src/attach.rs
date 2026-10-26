//! Files sent with a message. Images (png, jpeg, gif, webp) go to the model as real attachments
//! (staged in the profile's image cache, which `prompt.submit` requires); every file is also
//! named in the message as `[attached: <path>]`, a path on the gateway's machine the agent can
//! read. A local file is used in place unless it lives in a temporary folder (a dragged
//! screenshot) or its path has Unicode spaces (`plain`): then it is copied to the profile's
//! `attachments/`; over SSH files are uploaded.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::connection::{state_dir, Connection};
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

/// `name` with plain spaces for Unicode ones. Pi's tools make them plain in every path they are
/// given and then miss the file (a Russian macOS screenshot is "Снимок экрана — … в 00.32.36"
/// with U+00A0 twice), so such a file is copied under this name rather than named in place.
fn plain(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '\u{00A0}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => ' ',
            c => c,
        })
        .collect()
}

/// Blocking: call off the UI thread.
/// `machine`: the project's work is on that machine, not the agent's (this Mac's agent working on
/// a server): the files go to its temp folder, where the agent's tools look — one-off, gone with
/// the server's next reboot.
pub fn prepare(
    conn: &Connection,
    target: &Target,
    machine: Option<&str>,
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
            .map(|n| plain(&n.to_string_lossy()))
            .unwrap_or_else(|| "file".into());
        let bytes = std::fs::read(file).with_context(|| format!("read {name}"))?;
        let path = if let Some(m) = machine {
            let dest = format!("/tmp/bench-attachments/{stamp}-{name}");
            rest::upload_file_to(conn, profile, Some(m), &dest, &bytes)?;
            dest
        } else if *target == Target::Local
            && !is_temporary(file)
            && plain(&file.to_string_lossy()) == file.to_string_lossy()
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screenshot_name_gets_plain_spaces() {
        let name = "Снимок\u{00A0}экрана\u{00A0}— 2026-10-27 в\u{00A0}00.32.36.png";
        assert_eq!(plain(name), "Снимок экрана — 2026-10-27 в 00.32.36.png");
        assert_eq!(plain("shot 1.png"), "shot 1.png");
    }
}
