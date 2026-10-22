//! Where the gateway lives and how to authenticate to it.
//!
//! * Local: `hermes gateway ensure --json` names the endpoint; Hermes' own Python
//!   (`hermes_cli.gateway_client._session_ticket`) mints one-shot tickets, sent as a
//!   WebSocket subprotocol or the `X-Hermes-Gateway-Ticket` header — exactly what
//!   Hermes Desktop does.
//! * SSH: like Desktop's remote mode — a dedicated `hermes serve --isolated --port 0`
//!   on the host, started with our own session token, reached through `ssh -L`.
//!   Port and token are remembered so the next launch reattaches.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::{Value, json};
use tungstenite::WebSocket;
use tungstenite::client::IntoClientRequest;

use crate::model::Target;

const TICKET_SCRIPT: &str = r#"
import json, sys
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
with redirect_stdout(sys.stderr):
    from hermes_cli.gateway_client import _session_ticket
for line in sys.stdin:
    try:
        req = json.loads(line)
        ep = req['endpoint']
        with redirect_stdout(sys.stderr):
            t = _session_ticket(Path(ep['profile_id']), SimpleNamespace(**{'control_home': None, **ep}),
                                purpose=req['purpose'], scope='host' if req['purpose'] == 'interactive' else None)
        print(json.dumps({'ticket': t}), flush=True)
    except Exception as exc:
        print(json.dumps({'error': str(exc)}), flush=True)
"#;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

/// A long-lived Python helper that mints tickets on request (one per line).
struct TicketHelper {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl TicketHelper {
    fn start() -> Result<Self> {
        let mut child = Command::new(home().join(".hermes/hermes-agent/venv/bin/python"))
            .args(["-I", "-u", "-c", TICKET_SCRIPT])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("start Hermes ticket helper")?;
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Ok(Self {
            _child: child,
            stdin,
            stdout,
        })
    }

    fn mint(&mut self, endpoint: &Value, purpose: &str) -> Result<String> {
        writeln!(
            self.stdin,
            "{}",
            json!({"endpoint": endpoint, "purpose": purpose})
        )?;
        let mut line = String::new();
        self.stdout.read_line(&mut line)?;
        let v: Value = serde_json::from_str(&line).context("ticket helper reply")?;
        v["ticket"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("ticket: {}", v["error"]))
    }
}

/// An `ssh -N -L` child; dropped => tunnel closed.
struct Tunnel(Child);

impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

enum Auth {
    Ticket {
        endpoint: Value,
        helper: Mutex<TicketHelper>,
    },
    Token(String),
}

pub struct Connection {
    /// `http://127.0.0.1:<port>` reachable from this machine.
    pub base: String,
    auth: Auth,
    _tunnel: Option<Tunnel>,
}

impl Connection {
    pub fn open(target: &Target) -> Result<Self> {
        match target {
            Target::Local => Self::open_local(),
            Target::Ssh(host) => Self::open_ssh(host),
        }
    }

    /// Ticket mode speaks the canonical protocol (prompts carry profile + submission id).
    pub fn canonical(&self) -> bool {
        matches!(self.auth, Auth::Ticket { .. })
    }

    fn open_local() -> Result<Self> {
        // A gateway that is restarting (draining a running turn, starting up) gets time to come back.
        let deadline = Instant::now() + Duration::from_secs(180);
        let v = loop {
            let out = Command::new(home().join(".local/bin/hermes"))
                .args(["gateway", "ensure", "--json"])
                .output()
                .context("run `hermes gateway ensure`")?;
            let v: Value =
                serde_json::from_slice(&out.stdout).context("parse `gateway ensure` output")?;
            match v["state"].as_str() {
                Some("ready") => break v,
                Some("draining" | "starting") if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_secs(3))
                }
                other => bail!("local gateway is not ready: {}", other.unwrap_or("?")),
            }
        };
        let endpoint = v["endpoint"].clone();
        let base = endpoint["api_origin"]
            .as_str()
            .ok_or_else(|| anyhow!("no api_origin"))?
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            base,
            auth: Auth::Ticket {
                endpoint,
                helper: Mutex::new(TicketHelper::start()?),
            },
            _tunnel: None,
        })
    }

    pub fn websocket(&self) -> Result<WebSocket<TcpStream>> {
        let host = self.base.trim_start_matches("http://").to_string();
        let req = match &self.auth {
            Auth::Ticket { endpoint, helper } => {
                let ticket = helper.lock().unwrap().mint(endpoint, "interactive")?;
                let mut req =
                    format!("ws://{host}/api/ws?native_dial=unminted").into_client_request()?;
                req.headers_mut().insert(
                    "Sec-WebSocket-Protocol",
                    format!("hermes-gateway-v1, hermes-gateway-ticket.{ticket}").parse()?,
                );
                req
            }
            Auth::Token(token) => {
                format!("ws://{host}/api/ws?token={token}").into_client_request()?
            }
        };
        let stream = TcpStream::connect(&host)?;
        let (ws, _) = tungstenite::client(req, stream).map_err(|e| anyhow!("websocket: {e}"))?;
        ws.get_ref()
            .set_read_timeout(Some(Duration::from_millis(30)))?;
        Ok(ws)
    }

    /// Auth header for one REST call scoped to `profile`.
    pub fn rest_auth(&self, profile: &str) -> Result<(&'static str, String)> {
        match &self.auth {
            Auth::Token(token) => Ok(("X-Hermes-Session-Token", token.clone())),
            Auth::Ticket { endpoint, helper } => {
                let mut ep = endpoint.clone();
                if !profile.is_empty() && profile != "default" {
                    let root = endpoint["profile_id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    ep["profile_id"] = json!(format!("{root}/profiles/{profile}"));
                    ep["control_home"] = json!(root);
                }
                Ok((
                    "X-Hermes-Gateway-Ticket",
                    helper.lock().unwrap().mint(&ep, "native-http")?,
                ))
            }
        }
    }

    // ---- SSH -------------------------------------------------------------

    fn open_ssh(host: &str) -> Result<Self> {
        match host_check(host) {
            HostCheck::Ready => {}
            HostCheck::Unreachable => bail!(
                "{host}: SSH could not connect (offline, or macOS has not allowed local network access)"
            ),
            HostCheck::NoHermes => bail!("{host}: no working Hermes at ~/.local/bin/hermes"),
        }
        let state_path = state_dir().join(format!("ssh-{host}.json"));
        // Reattach to the backend we started last time, if it still answers.
        if let Ok(saved) = std::fs::read(&state_path)
            .map_err(anyhow::Error::from)
            .and_then(|b| Ok(serde_json::from_slice::<Value>(&b)?))
        {
            if let (Some(port), Some(token)) = (saved["port"].as_u64(), saved["token"].as_str()) {
                // The saved token must still open the WebSocket (a restarted backend rejects it with 403).
                if let Ok(conn) = Self::attach_ssh(host, port as u16, token) {
                    if conn.websocket().is_ok() {
                        return Ok(conn);
                    }
                }
            }
        }
        let (port, token) = spawn_remote_backend(host)?;
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
            auth: Auth::Token(token.into()),
            _tunnel: Some(tunnel),
        })
    }
}

pub fn state_dir() -> PathBuf {
    home().join("Library/Application Support/hermes-gpui")
}

fn write_private(path: &PathBuf, text: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

fn hex(n: usize) -> String {
    let mut s = String::new();
    while s.len() < n {
        s.push_str(&uuid::Uuid::new_v4().simple().to_string());
    }
    s.truncate(n);
    s
}

/// Start `hermes serve --isolated --port 0` on the host with a fresh token
/// (passed on stdin, written 0600 where `serve` expects it) and read its port.
fn spawn_remote_backend(host: &str) -> Result<(u16, String)> {
    let token = hex(64);
    let dir = format!("$HOME/.hermes/desktop-ssh/{}", hex(32));
    let file = format!("{dir}/{}.token", hex(16));
    let script = format!(
        r#"umask 077; mkdir -p "{dir}"; chmod 700 "{dir}"; cat > "{file}"; cd ~
nohup setsid bash -lc '$HOME/.local/bin/hermes serve --port 0 --skip-build --isolated --ssh-session-token-file "{file}"' > "{dir}/serve.log" 2>&1 < /dev/null &
for i in $(seq 1 150); do p=$(grep -o 'HERMES_DASHBOARD_READY port=[0-9]*' "{dir}/serve.log" | head -1); [ -n "$p" ] && {{ echo "$p"; exit 0; }}; sleep 1; done
tail -3 "{dir}/serve.log"; exit 1"#
    );
    let mut child = Command::new("ssh")
        .args(["-o", "BatchMode=yes", host, "bash", "-c"])
        .arg(shell_quote(&script))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("ssh to host")?;
    child.stdin.take().unwrap().write_all(token.as_bytes())?;
    let out = child.wait_with_output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let port = text
        .split("port=")
        .nth(1)
        .and_then(|s| s.trim().split_whitespace().next())
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| {
            anyhow!(
                "{host}: backend did not start: {}{}",
                text.trim(),
                String::from_utf8_lossy(&out.stderr).trim()
            )
        })?;
    Ok((port, token))
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `hermes gateway restart` locally or on the SSH host (the messaging gateway service).
pub fn restart_gateway(target: &Target) -> Result<()> {
    let out = match target {
        Target::Local => Command::new(home().join(".local/bin/hermes"))
            .args(["gateway", "restart"])
            .output()?,
        Target::Ssh(host) => Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                host,
                "bash -lc '\"$HOME/.local/bin/hermes\" gateway restart'",
            ])
            .output()?,
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!(
            "{}",
            err.lines()
                .last()
                .unwrap_or("hermes gateway restart failed")
        );
    }
    Ok(())
}

/// Whether `host` answers over SSH and has a working Hermes (a stale wrapper is not enough).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCheck {
    Ready,
    /// SSH itself failed (exit 255): offline, no route, no network permission yet.
    Unreachable,
    /// Reached the host, but Hermes is missing or broken there.
    NoHermes,
}

/// Checks a host over SSH; the result is also appended to `hosts.log` for diagnosis.
pub fn host_check(host: &str) -> HostCheck {
    let status = Command::new("/usr/bin/ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=5",
            host,
            "timeout 30 \"$HOME/.local/bin/hermes\" --version >/dev/null 2>&1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let result = match &status {
        Ok(s) if s.success() => HostCheck::Ready,
        Ok(s) if s.code() == Some(255) => HostCheck::Unreachable,
        Ok(_) => HostCheck::NoHermes,
        Err(_) => HostCheck::Unreachable,
    };
    let _ = std::fs::create_dir_all(state_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir().join("hosts.log"))
    {
        use std::io::Write as _;
        let _ = writeln!(f, "{host}: {result:?} ({status:?})");
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
