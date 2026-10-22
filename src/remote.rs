//! Files on a server of `~/.ssh/config`, the way Transmit does it: plain ssh (nothing installed
//! there), a file is opened as a local copy in the cache and every change of that copy — saved
//! in the editor or made by the agent — is written back to the server.
//!
//! Blocking: call off the UI thread. Commands go through the shared ssh connection (`ssh.rs`).

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};

use crate::ssh::{q, run};

/// Larger files are not opened in the editor (download stays explicit: `scp`, Transmit…).
pub const MAX_OPEN: u64 = 50 * 1024 * 1024;

/// The login directory (where `ssh <alias>` starts).
pub fn home_dir(alias: &str) -> Result<String> {
    let out = run(alias, "pwd", None, Duration::from_secs(20))?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// `dir`'s entries: (name, full path, is directory) — folders first, then by name; hidden ones
/// included.
pub fn list_dir(alias: &str, dir: &str) -> Result<Vec<(String, String, bool)>> {
    let out = run(
        alias,
        &format!("ls -1Ap -- {}", q(dir)),
        None,
        Duration::from_secs(30),
    )?;
    let base = dir.trim_end_matches('/');
    let mut rows: Vec<(String, String, bool)> = String::from_utf8_lossy(&out)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let is_dir = l.ends_with('/');
            let name = l.trim_end_matches('/').to_string();
            (name.clone(), format!("{base}/{name}"), is_dir)
        })
        .collect();
    rows.sort_by_key(|(n, _, d)| (!d, n.to_lowercase()));
    Ok(rows)
}

/// Where a server's file is kept while open on this Mac.
pub fn cache_path(alias: &str, remote: &str) -> PathBuf {
    crate::ssh::home()
        .join("Library/Caches/hermes-gpui/remote")
        .join(alias.replace('/', "_"))
        .join(remote.trim_start_matches('/'))
}

/// The file's modification time on the server, in seconds (GNU stat, else BSD stat).
pub fn mtime(alias: &str, remote: &str) -> Result<i64> {
    let f = q(remote);
    let out = run(
        alias,
        &format!("stat -c %Y {f} 2>/dev/null || stat -f %m {f}"),
        None,
        Duration::from_secs(20),
    )?;
    let text = String::from_utf8_lossy(&out);
    text.trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("stat {remote}: {}", text.trim()))
}

/// What `upload_checked` did.
pub enum Upload {
    /// Written; the file's new modification time.
    Done(i64),
    /// Not written: the file changed on the server since `expected`.
    Conflict,
}

/// Write the file unless it changed on the server since it was read (`expected`: its
/// modification time then; None = overwrite whatever is there).
pub fn upload_checked(
    alias: &str,
    remote: &str,
    bytes: Vec<u8>,
    expected: Option<i64>,
) -> Result<Upload> {
    if let Some(expected) = expected {
        let now = mtime(alias, remote)?;
        if now != expected {
            return Ok(Upload::Conflict);
        }
    }
    upload(alias, remote, bytes)?;
    Ok(Upload::Done(mtime(alias, remote).unwrap_or_default()))
}

/// The server file a cache path is the local copy of: (alias, path on the server).
pub fn origin(local: &str) -> Option<(String, String)> {
    let base = cache_path("", "");
    let rest = local.strip_prefix(base.to_str()?)?.trim_start_matches('/');
    let (alias, path) = rest.split_once('/')?;
    Some((alias.to_string(), format!("/{path}")))
}

/// Copy the server's file into the cache; returns the local path and the file's modification
/// time on the server (to notice later changes made there).
pub fn download(alias: &str, remote: &str) -> Result<(PathBuf, i64)> {
    let server_mtime = mtime(alias, remote)?;
    let size = run(
        alias,
        &format!("wc -c < {}", q(remote)),
        None,
        Duration::from_secs(20),
    )?;
    let size: u64 = String::from_utf8_lossy(&size).trim().parse().unwrap_or(0);
    if size > MAX_OPEN {
        bail!("{} MB is too large to open here", size / 1024 / 1024);
    }
    let bytes = run(
        alias,
        &format!("cat {}", q(remote)),
        None,
        Duration::from_secs(300),
    )?;
    let local = cache_path(alias, remote);
    std::fs::create_dir_all(local.parent().unwrap())?;
    std::fs::write(&local, bytes)?;
    Ok((local, server_mtime))
}

/// Write `bytes` to the server's file (`cat >` keeps its owner and mode).
pub fn upload(alias: &str, remote: &str, bytes: Vec<u8>) -> Result<()> {
    run(
        alias,
        &format!("cat > {}", q(remote)),
        Some(bytes),
        Duration::from_secs(300),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_of_cache_paths() {
        let local = cache_path("dev", "/etc/nginx/conf.d/site.conf");
        assert_eq!(
            origin(local.to_str().unwrap()),
            Some(("dev".into(), "/etc/nginx/conf.d/site.conf".into()))
        );
        assert_eq!(origin("/Users/x/Desktop/a.txt"), None);
    }
}
