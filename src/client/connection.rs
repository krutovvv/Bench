//! Where the agent lives and how to reach it: `launch::ensure()` brings it up as its machine's
//! service and names its port — on this Mac reached directly, on a server through `ssh -L`.
//! Either way it is a session token, sent as `?token=` on the WebSocket and as the
//! `X-Bench-Token` header on REST.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use web_time::Instant;

use anyhow::{Context as _, Result, anyhow, bail};
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;

use crate::app::model::Target;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// An `ssh -N -L` child; dropped => tunnel closed.
struct Tunnel(Child);

impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

pub struct Connection {
    /// `http://127.0.0.1:<port>` reachable from this machine.
    pub base: String,
    token: String,
    _tunnel: Option<Tunnel>,
    /// What answers its REST calls when no real agent is there (`client/fake.rs`).
    pub(crate) fake: Option<std::sync::Arc<dyn crate::client::fake::FakeAgent>>,
}

impl Connection {
    /// The connection to a fake agent: nothing is opened.
    pub fn to_fake(agent: std::sync::Arc<dyn crate::client::fake::FakeAgent>) -> Self {
        Self {
            base: String::new(),
            token: String::new(),
            _tunnel: None,
            fake: Some(agent),
        }
    }

    pub fn open(target: &Target) -> Result<Self> {
        match target {
            Target::Local => Self::open_local(),
            Target::Ssh(host) => Self::open_ssh(host),
        }
    }

    fn open_local() -> Result<Self> {
        let (port, token) = crate::client::launch::ensure(&Target::Local)?;
        Ok(Self {
            base: format!("http://127.0.0.1:{port}"),
            token,
            _tunnel: None,
            fake: None,
        })
    }

    pub fn websocket(&self) -> Result<WebSocket<TcpStream>> {
        let host = self.base.trim_start_matches("http://").to_string();
        let req = format!("ws://{host}/api/ws?token={}", self.token).into_client_request()?;
        let stream = TcpStream::connect(&host)?;
        let (ws, _) = tungstenite::client(req, stream).map_err(|e| anyhow!("websocket: {e}"))?;
        ws.get_ref()
            .set_read_timeout(Some(Duration::from_millis(30)))?;
        Ok(ws)
    }

    /// The header that lets a REST call in: the agent's session token.
    pub fn auth(&self) -> (&'static str, String) {
        ("X-Bench-Token", self.token.clone())
    }

    // ---- SSH -------------------------------------------------------------

    fn open_ssh(host: &str) -> Result<Self> {
        match host_check(host) {
            HostCheck::Ready => {}
            HostCheck::Unreachable => bail!(
                "{host}: SSH could not connect (offline, or macOS has not allowed local network access)"
            ),
            HostCheck::NoNode => bail!("{host}: Bench's agent needs Node.js 22.18 or newer there"),
        }
        // Always through `ensure`, as on this Mac: it is what keeps the server's agent its
        // machine's service, of this app's build (a saved port reused without it once left an
        // agent of the old kind running).
        let (port, token) = crate::client::launch::ensure(&Target::Ssh(host.to_string()))?;
        Self::attach_ssh(host, port, &token)
    }

    fn attach_ssh(host: &str, remote_port: u16, token: &str) -> Result<Self> {
        let local_port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let child = Command::new("ssh")
            .args([
                "-N",
                "-o",
                "BatchMode=yes",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ServerAliveInterval=15",
            ])
            .arg("-L")
            .arg(format!("{local_port}:127.0.0.1:{remote_port}"))
            .arg(host)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("start ssh tunnel")?;
        let tunnel = Tunnel(child);
        let base = format!("http://127.0.0.1:{local_port}");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if TcpStream::connect(("127.0.0.1", local_port)).is_ok() {
                let ok = ureq::get(&format!("{base}/api/status"))
                    .set("X-Bench-Token", token)
                    .timeout(Duration::from_secs(5))
                    .call()
                    .is_ok();
                if ok {
                    break;
                }
            }
            if Instant::now() > deadline {
                bail!("{host}: backend on port {remote_port} did not answer");
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Ok(Self {
            base,
            token: token.into(),
            _tunnel: Some(tunnel),
            fake: None,
        })
    }
}

/// Bench's own state (last place, drafts, marks, settings): `~/.bench/ui`,
/// beside the agent's home.
pub fn state_dir() -> PathBuf {
    STATE_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| crate::client::launch::agent_home().join("ui"))
}

static STATE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Keep this run's state somewhere else (the demo: a folder of its own, so nothing it does —
/// folds, marks, tabs, the last thread — touches the person's).
pub fn use_state_dir(dir: PathBuf) {
    let _ = STATE_DIR.set(dir);
}

/// Bench kept its state in `~/Library/Application Support/bench` before (and, as hermes-gpui,
/// in `…/hermes-gpui`): the first of those found is copied to `state_dir()` the first time, not
/// moved — an older build may still run from it.
pub fn migrate_state() {
    let new = state_dir();
    if new.exists() {
        return;
    }
    let support = home().join("Library/Application Support");
    let Some(old) = [support.join("bench"), support.join("hermes-gpui")]
        .into_iter()
        .find(|d| d.is_dir())
    else {
        return;
    };
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::process::Command::new("cp")
        .arg("-R")
        .arg(&old)
        .arg(&new)
        .status();
    // The hosts' saved ports and tokens named the backends started before (Hermes): the agent is
    // started afresh on each host.
    if let Ok(dir) = std::fs::read_dir(&new) {
        for f in dir.flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            if name.starts_with("ssh-") && name.ends_with(".json") {
                let _ = std::fs::remove_file(f.path());
            }
        }
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// What a restart did.
pub enum Restart {
    Done,
    /// Kept for the UI's "busy" branch; the agent's runs are durable, so it never refuses.
    #[allow(dead_code)]
    Busy(String),
}

/// Restarts the agent (its unfinished runs continue after the start).
pub fn restart_agent(target: &Target, _interrupt: bool) -> Result<Restart> {
    crate::client::launch::restart(target)?;
    Ok(Restart::Done)
}

/// The last `lines` of a log, reading only its end (`bytes` at most) — the agent's log grows
/// while it works, and this runs on a timer.
fn tail_last(path: &std::path::Path, bytes: u64, lines: usize) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(len) = std::fs::metadata(path).map(|m| m.len()) else {
        return String::new();
    };
    let start = len.saturating_sub(bytes);
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&buf).to_string();
    // The first line may be cut in the middle of it: it is not a line.
    if start == 0 {
        text
    } else {
        match text.split_once('\n') {
            Some((_, rest)) => rest.to_string(),
            None => String::new(),
        }
    }
    .lines()
    .rev()
    .take(lines)
    .collect::<Vec<_>>()
    .into_iter()
    .rev()
    .collect::<Vec<_>>()
    .join("\n")
}

/// The last lines of the agent's own log (`~/.bench/server.log` where it runs): what it says
/// before the status has anything to say. Silent when there is no log yet or the host is away.
pub fn agent_log(target: &Target, lines: usize) -> Vec<String> {
    if let Some(fake) = crate::client::fake::fake() {
        return fake.agent_log();
    }
    let raw = match target {
        Target::Local => tail_last(
            &crate::client::launch::agent_home().join("server.log"),
            64 * 1024,
            lines,
        ),
        Target::Ssh(host) => match crate::system::ssh::run(
            host,
            &format!("tail -n {lines} ~/.bench/server.log 2>/dev/null; true"),
            None,
            Duration::from_secs(5),
        ) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(_) => return vec![],
        },
    };
    raw.lines()
        .rev()
        .take(lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|s| s.to_string())
        .collect()
}

/// Run `git -C dir args…` where the agent lives (this Mac, or the SSH host); stdout trimmed.
pub fn git(target: &Target, dir: &str, args: &[&str]) -> Result<String> {
    if let Some(fake) = crate::client::fake::fake() {
        return fake.git(dir, args);
    }
    let out = match target {
        Target::Local => Command::new("git").arg("-C").arg(dir).args(args).output()?,
        Target::Ssh(host) => {
            let quoted: Vec<String> = std::iter::once("git".to_string())
                .chain(["-C", dir].iter().chain(args).map(|a| shell_quote(a)))
                .collect();
            Command::new("ssh")
                .args(["-o", "BatchMode=yes", host, &quoted.join(" ")])
                .stdin(Stdio::null())
                .output()?
        }
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = err
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("hint:"))
            .collect::<Vec<_>>()
            .join(" ");
        bail!(
            "git {}: {}",
            args.first().unwrap_or(&""),
            if msg.is_empty() { "failed".into() } else { msg }
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Whether `host` answers over SSH and has a Node the agent can run on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCheck {
    Ready,
    /// SSH itself failed: offline, no route, no network permission yet.
    Unreachable,
    /// Reached the host, but its Node is missing or too old.
    NoNode,
}

/// Checks a host over SSH; the result is also appended to `hosts.log` for diagnosis.
pub fn host_check(host: &str) -> HostCheck {
    let result = match crate::client::launch::node_ready(&Target::Ssh(host.to_string())) {
        Ok(true) => HostCheck::Ready,
        Ok(false) => HostCheck::NoNode,
        Err(_) => HostCheck::Unreachable,
    };
    let _ = std::fs::create_dir_all(state_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir().join("hosts.log"))
    {
        let _ = writeln!(f, "{host}: {result:?}");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_lines_of_a_log() {
        let dir = std::env::temp_dir().join(format!("bench-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("server.log");
        // A long line before them, so the read starts in the middle of it.
        let long = "x".repeat(200 * 1024);
        let body = (1..=40)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, format!("{long}\n{body}\n")).unwrap();
        assert_eq!(tail_last(&path, 64 * 1024, 3), "line 38\nline 39\nline 40");
        assert_eq!(
            tail_last(&std::path::Path::new("/bench-nope/server.log"), 1024, 3),
            "",
            "no log is no lines"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
