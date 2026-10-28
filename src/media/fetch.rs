//! Delivered files brought to this Mac and prepared for showing: downloaded through the agent
//! (or read in place), their Quick Look picture or video frame made off the UI thread.

use super::{Kind, file_name, kind};
use crate::app::model::Target;
use crate::client::connection::{Connection, state_dir};
use crate::client::rest;
use anyhow::{Result, bail};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn hash_of(parts: &[&str]) -> String {
    let mut h = DefaultHasher::new();
    parts.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// A delivered file, ready to show: where it is on this Mac and what to draw for it.
#[derive(Clone, Debug)]
pub struct Fetched {
    pub local: PathBuf,
    /// An image's size in pixels.
    pub size: Option<(f32, f32)>,
    /// A video's or document's Quick Look picture, with its size.
    pub thumb: Option<(PathBuf, (f32, f32))>,
    /// A Markdown file's text.
    pub text: Option<String>,
}

const MAX_DOWNLOAD: u64 = 512 << 20;

/// The file itself, when the agent runs on this Mac and it is there.
fn on_this_mac(target: &Target, path: &str) -> Option<PathBuf> {
    let p = match path.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(path),
    };
    (*target == Target::Local && p.is_file()).then_some(p)
}

/// A local file whose preview needs no work (no download, no Quick Look run): ready in the
/// same frame, so the chat lays out at its final height and nothing jumps.
pub fn fetch_now(target: &Target, path: &str) -> Option<Fetched> {
    let local = on_this_mac(target, path)?;
    if matches!(kind(path), Kind::Video | Kind::Doc) && !thumb_file(&local).is_file() {
        return None;
    }
    prepare(local, path).ok()
}

/// Bring a delivered file here (a local agent's files are read in place) and prepare its
/// preview. Blocking: call off the UI thread.
/// `machine`: the file is on that machine, not the agent's own (a project worked over SSH).
pub fn fetch(
    conn: &Connection,
    target: &Target,
    machine: Option<&str>,
    path: &str,
) -> Result<Fetched> {
    let local = match on_this_mac(target, path) {
        Some(p) => p,
        None => download(conn, target, machine, path)?,
    };
    prepare(local, path)
}

/// A local file prepared now (its video frame drawn), so the first paint already has it — for
/// the demo, which has no agent to fetch through.
pub fn warm(path: &str) {
    let _ = prepare(PathBuf::from(path), path);
}

fn prepare(local: PathBuf, path: &str) -> Result<Fetched> {
    let mut out = Fetched {
        local: local.clone(),
        size: None,
        thumb: None,
        text: None,
    };
    match kind(path) {
        Kind::Image => {
            out.size = imagesize::size(&local)
                .ok()
                .map(|s| (s.width as f32, s.height as f32));
        }
        Kind::Markdown => {
            let mut bytes = vec![];
            std::fs::File::open(&local)?
                .take(256 << 10)
                .read_to_end(&mut bytes)?;
            out.text = Some(String::from_utf8_lossy(&bytes).into_owned());
        }
        Kind::Video | Kind::Doc => out.thumb = quick_look(&local).ok(),
        Kind::Audio | Kind::File => {}
    }
    Ok(out)
}

fn download(
    conn: &Connection,
    target: &Target,
    machine: Option<&str>,
    path: &str,
) -> Result<PathBuf> {
    let bytes = rest::download(conn, machine, path, MAX_DOWNLOAD)?;
    let dir = state_dir()
        .join("media")
        .join(hash_of(&[&target.label(), path]));
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(file_name(path));
    std::fs::write(&file, bytes)?;
    Ok(file)
}

/// Where a file's Quick Look picture is kept: by path, size and modification time, so an
/// overwritten file gets a new one.
fn thumb_file(file: &Path) -> PathBuf {
    let meta = std::fs::metadata(file).ok();
    let stamp = meta
        .as_ref()
        .map(|m| {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            format!("{}-{t}", m.len())
        })
        .unwrap_or_default();
    let dir = state_dir()
        .join("media")
        .join("thumbs")
        .join(hash_of(&[&file.to_string_lossy(), &stamp]));
    dir.join(format!(
        "{}.png",
        file.file_name().unwrap_or_default().to_string_lossy()
    ))
}

/// The first frame or page as Quick Look draws it (`qlmanage -t`), with its pixel size.
fn quick_look(file: &Path) -> Result<(PathBuf, (f32, f32))> {
    let png = thumb_file(file);
    if !png.is_file() {
        let dir = png.parent().unwrap_or(Path::new("/tmp"));
        std::fs::create_dir_all(dir)?;
        let status = Command::new("qlmanage")
            .args(["-t", "-s", "720", "-o"])
            .arg(dir)
            .arg(file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !status.success() || !png.is_file() {
            bail!("no preview");
        }
    }
    let s = imagesize::size(&png)?;
    Ok((png, (s.width as f32, s.height as f32)))
}
