//! Where the agent lives and how to reach it: on this Mac `pi_agent::ensure()` starts it and
//! names its port; over SSH `pi_agent::ensure_remote()` does the same on the host, reached
//! through `ssh -L`. Either way it is a session token, sent as `?token=` on the WebSocket and as
//! the `X-Hermes-Session-Token` header on REST. Port and token of a host are remembered so the
//! next launch reattaches.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use web_time::Instant;

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::{Value, json};
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;

use crate::model::Target;

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
}

impl Connection {
    pub fn open(target: &Target) -> Result<Self> {
        match target {
            Target::Local => Self::open_local(),
            Target::Ssh(host) => Self::open_ssh(host),
        }
    }

    fn open_local() -> Result<Self> {
        let (port, token) = crate::pi_agent::ensure()?;
        Ok(Self {
            base: format!("http://127.0.0.1:{port}"),
            token,
            _tunnel: None,
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

    /// Auth header for one REST call (the profile and purpose no longer matter).
    pub fn rest_auth(&self, _profile: &str, _purpose: &str) -> Result<(&'static str, String)> {
        Ok(("X-Hermes-Session-Token", self.token.clone()))
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
        let state_path = state_dir().join(format!("ssh-{host}.json"));
        // Reattach to the agent we started last time, if it still answers.
        if let Ok(saved) = std::fs::read(&state_path)
            .map_err(anyhow::Error::from)
            .and_then(|b| Ok(serde_json::from_slice::<Value>(&b)?))
        {
            if let (Some(port), Some(token)) = (saved["port"].as_u64(), saved["token"].as_str()) {
                // The saved token must still open the WebSocket (a restarted agent rejects it).
                if let Ok(conn) = Self::attach_ssh(host, port as u16, token) {
                    if conn.websocket().is_ok() {
                        return Ok(conn);
                    }
                }
            }
        }
        let (port, token) = crate::pi_agent::ensure_remote(host)?;
        std::fs::create_dir_all(state_dir())?;
        write_private(
            &state_path,
            &json!({"port": port, "token": token}).to_string(),
        )?;
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
                    .set("X-Hermes-Session-Token", token)
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
        })
    }
}

/// Bench's own state (last place, drafts, marks, settings, SSH reattach files): `~/.bench/ui`,
/// beside the agent's home.
pub fn state_dir() -> PathBuf {
    crate::pi_agent::agent_home().join("ui")
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

fn write_private(path: &PathBuf, text: &str) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    // Readable by the owner only (where the system has Unix permissions).
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path)?;
    f.write_all(text.as_bytes())?;
    Ok(())
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
pub fn restart_gateway(target: &Target, _interrupt: bool) -> Result<Restart> {
    match target {
        Target::Local => crate::pi_agent::restart()?,
        Target::Ssh(host) => {
            let _ = Command::new("ssh")
                .args(["-o", "BatchMode=yes", host, "kill $(sed -n 's/.*\"pid\":\\([0-9]*\\).*/\\1/p' ~/.bench/server.json) 2>/dev/null; true"])
                .stdin(Stdio::null())
                .output();
            let _ = std::fs::remove_file(state_dir().join(format!("ssh-{host}.json")));
        }
    }
    Ok(Restart::Done)
}

/// A group of chats with no project of its own ("Audit X", a few deep researches): a plain
/// folder `~/Research/<name>` where the gateway lives, made if missing. The sidebar groups the
/// chats by it, and what they write lands there. Returns the folder's absolute path.
pub fn group_dir(target: &Target, name: &str) -> Result<String> {
    let name = name.trim().replace('/', "-");
    if name.is_empty() || name.starts_with('.') {
        bail!("not a group name: {name:?}");
    }
    match target {
        Target::Local => {
            let dir = PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("Research")
                .join(&name);
            std::fs::create_dir_all(&dir)?;
            Ok(dir.to_string_lossy().into_owned())
        }
        Target::Ssh(host) => {
            let cmd = format!(
                "d=\"$HOME\"/Research/{} && mkdir -p \"$d\" && cd \"$d\" && pwd",
                shell_quote(&name)
            );
            let out = Command::new("ssh")
                .args(["-o", "BatchMode=yes", host, &cmd])
                .stdin(Stdio::null())
                .output()?;
            if !out.status.success() {
                bail!("mkdir: {}", String::from_utf8_lossy(&out.stderr).trim());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        }
    }
}

/// Run `git -C dir args…` where the gateway lives (this Mac, or the SSH host); stdout trimmed.
pub fn git(target: &Target, dir: &str, args: &[&str]) -> Result<String> {
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
    let result = match crate::pi_agent::remote_node(host) {
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

/// `Host` entries from ~/.ssh/config, without wildcards.
pub fn ssh_hosts() -> Vec<String> {
    let text = std::fs::read_to_string(home().join(".ssh/config")).unwrap_or_default();
    let mut hosts = vec![];
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line
            .strip_prefix("Host ")
            .or_else(|| line.strip_prefix("Host\t"))
        else {
            continue;
        };
        for h in rest.split_whitespace() {
            if !h.contains(['*', '?', '!']) && !hosts.contains(&h.to_string()) {
                hosts.push(h.to_string());
            }
        }
    }
    hosts
}
