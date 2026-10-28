//! The agent Bench talks to: bench-agent (`agent/` in this repo), a server on Pi Durable. Bench
//! installs and starts it on this Mac and, over SSH, on another machine, then speaks its
//! protocol (JSON-RPC over `/api/ws?token=`, REST under `/api/` with `X-Bench-Token`).
//!
//! * Its home is `~/.bench` on each machine: `bench.sqlite`, `pi.sqlite`, `agents/`, `projects/`,
//!   `extensions/`, and `app/` — the installed server, copied from the sources this app carries
//!   (`Contents/Resources/agent`, or this repo's `agent/` for a dev build); `npm ci` runs there
//!   when `package-lock.json` changed. Node 22.18+ is needed.
//! * The server writes its port and token to `~/.bench/server.json`. A running server is reused;
//!   one built from other sources is restarted when idle (its conversations are durable).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::Value;
use web_time::Instant;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

pub fn agent_home() -> PathBuf {
    std::env::var("BENCH_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join(".bench"))
}

/// The agent's sources this app carries.
fn bundled_source() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BENCH_AGENT_SRC") {
        return Some(PathBuf::from(dir));
    }
    let exe = std::env::current_exe().ok()?;
    let resources = exe.parent()?.parent()?.join("Resources/agent");
    if resources.join("package.json").exists() {
        return Some(resources);
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("agent");
    repo.join("package.json").exists().then_some(repo)
}

/// What gets installed: the package files and the folders the server runs from.
const PARTS: [&str; 4] = ["src", "scripts", "package.json", "package-lock.json"];

fn node() -> Result<PathBuf> {
    let candidates = [
        home().join(".local/bin/node"),
        PathBuf::from("/opt/homebrew/bin/node"),
        PathBuf::from("/usr/local/bin/node"),
    ];
    let found = candidates.into_iter().find(|p| p.exists()).or_else(|| {
        let out = Command::new("zsh")
            .args(["-lc", "command -v node"])
            .output()
            .ok()?;
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!p.is_empty()).then(|| PathBuf::from(p))
    });
    let node =
        found.ok_or_else(|| anyhow!("Bench's agent needs Node.js 22.18 or newer (not found)"))?;
    let v = Command::new(&node).arg("--version").output()?;
    let v = String::from_utf8_lossy(&v.stdout);
    if !node_ok(&v) {
        bail!(
            "Bench's agent needs Node.js 22.18 or newer ({} is {})",
            node.display(),
            v.trim()
        );
    }
    Ok(node)
}

fn node_ok(version: &str) -> bool {
    let mut parts = version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(|n| n.parse::<u32>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    major > 22 || (major == 22 && minor >= 18)
}

/// A hash of the sources, to tell an outdated install or server.
fn source_hash(dir: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut files = vec![];
    for part in PARTS {
        let p = dir.join(part);
        if p.is_dir() {
            walk(&p, &mut files);
        } else {
            files.push(p);
        }
    }
    files.sort();
    let mut h = Sha256::new();
    for f in files {
        if let Ok(bytes) = std::fs::read(&f) {
            h.update(
                f.strip_prefix(dir)
                    .unwrap_or(&f)
                    .to_string_lossy()
                    .as_bytes(),
            );
            h.update(&bytes);
        }
    }
    format!("{:x}", h.finalize())[..16].to_string()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The build of the agent this app carries (what `ensure_remote` installs on a server).
pub fn build() -> Option<String> {
    source().ok().map(|(_, hash)| hash)
}

fn source() -> Result<(PathBuf, String)> {
    let src = bundled_source().ok_or_else(|| anyhow!("this build carries no agent sources"))?;
    let hash = source_hash(&src);
    Ok((src, hash))
}

// ---- this Mac --------------------------------------------------------------------

/// Copies the sources to `~/.bench/app` and installs its packages when needed.
fn install() -> Result<(PathBuf, String)> {
    let (src, hash) = source()?;
    let app = agent_home().join("app");
    // Its packages count as installed only once `npm ci` went through to the end: it writes the
    // lock it installed (`.lock-installed`), as the server's install does. An install cut short
    // (Bench quit while it ran) left node_modules half full and was never tried again — the agent
    // then died at every start on a missing module.
    let marker = app.join(".lock-installed");
    let installed = std::fs::read(&marker).ok();
    if source_hash(&app) != hash || installed != std::fs::read(src.join("package-lock.json")).ok() {
        std::fs::create_dir_all(&app)?;
        let lock_changed = installed != std::fs::read(src.join("package-lock.json")).ok();
        for part in PARTS {
            let _ = std::fs::remove_dir_all(app.join(part));
            let _ = std::fs::remove_file(app.join(part));
            let status = Command::new("cp")
                .arg("-R")
                .arg(src.join(part))
                .arg(app.join(part))
                .status()?;
            if !status.success() {
                bail!("copy {part} into {}", app.display());
            }
        }
        if lock_changed || !app.join("node_modules").exists() {
            let node = node()?;
            let out = Command::new(node.with_file_name("npm"))
                .args(["ci", "--omit=dev", "--no-audit", "--no-fund"])
                .current_dir(&app)
                .env(
                    "PATH",
                    format!("{}:/usr/bin:/bin", node.parent().unwrap().display()),
                )
                .output()
                .context("run npm ci for the agent")?;
            if !out.status.success() {
                bail!(
                    "npm ci failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .last()
                        .unwrap_or("")
                );
            }
            std::fs::copy(app.join("package-lock.json"), &marker)?;
        }
    }
    Ok((app, hash))
}

fn saved() -> Option<Value> {
    serde_json::from_slice(&std::fs::read(agent_home().join("server.json")).ok()?).ok()
}

fn status(port: u16, token: &str) -> Option<Value> {
    ureq::get(&format!("http://127.0.0.1:{port}/api/status"))
        .set("X-Bench-Token", token)
        .timeout(Duration::from_secs(3))
        .call()
        .ok()?
        .into_json()
        .ok()
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The PATH the agent runs with. An app opened from the Finder gets only the system folders, so
/// the agent's MCP servers (`npx …`) and its bash (`cargo`, `brew`) found nothing: node's own
/// folder, Homebrew's and cargo's go first.
fn agent_path(node: &Path) -> String {
    let first = [
        node.parent().map(Path::to_path_buf),
        Some(home().join(".cargo/bin")),
        Some(home().join(".local/bin")),
        Some(PathBuf::from("/opt/homebrew/bin")),
        Some(PathBuf::from("/usr/local/bin")),
    ];
    let rest = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin:/usr/sbin:/sbin".into());
    let mut dirs: Vec<String> = Vec::new();
    let all = first
        .into_iter()
        .flatten()
        .map(|d| d.to_string_lossy().into_owned());
    for d in all.chain(rest.split(':').map(str::to_string)) {
        if !d.is_empty() && !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    dirs.join(":")
}

/// The agent outlived the app that started it: its parent is launchd now. macOS judges its
/// local-network access by that app, so such an agent fails every request to a LAN model at once
/// ("Connection error.", in a few milliseconds) until it is started again by a living app.
fn orphaned(pid: u32) -> bool {
    Command::new("ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .ok()
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
}

fn stop() {
    if let Some(pid) = saved().and_then(|v| v["pid"].as_u64()) {
        let _ = Command::new("kill").arg(pid.to_string()).status();
        let deadline = Instant::now() + Duration::from_secs(5);
        while pid_alive(pid as u32) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Stops the server and starts it again (its unfinished runs continue after the start).
pub fn restart() -> Result<()> {
    stop();
    ensure().map(|_| ())
}

/// The running agent's port and token on this Mac, installing and starting it first if needed.
pub fn ensure() -> Result<(u16, String)> {
    let (app, hash) = install()?;
    if let Some(v) = saved() {
        if let (Some(port), Some(token)) = (v["port"].as_u64(), v["token"].as_str()) {
            if let Some(st) = status(port as u16, token) {
                let idle = st["active"].as_u64().unwrap_or(0) == 0;
                // An orphan is restarted even mid-run: its runs are durable, and a bash call cut
                // off (ship.sh, which quit the previous app) comes back to the model as
                // `interrupted`, never rerun.
                let orphan = v["pid"].as_u64().is_some_and(|pid| orphaned(pid as u32));
                if !orphan && (v["build"].as_str() == Some(hash.as_str()) || !idle) {
                    return Ok((port as u16, token.to_string()));
                }
                stop();
            }
        }
    }
    let node = node()?;
    let log = std::fs::File::create(agent_home().join("server.log"))?;
    let before = std::fs::metadata(agent_home().join("server.json"))
        .and_then(|m| m.modified())
        .ok();
    Command::new("nohup")
        .arg(&node)
        .arg("src/server.ts")
        .current_dir(&app)
        .env("BENCH_AGENT_BUILD", &hash)
        .env("PATH", agent_path(&node))
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("start the agent")?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let fresh = std::fs::metadata(agent_home().join("server.json"))
            .and_then(|m| m.modified())
            .ok()
            != before;
        if let (true, Some(v)) = (fresh, saved()) {
            if let (Some(port), Some(token)) = (v["port"].as_u64(), v["token"].as_str()) {
                if status(port as u16, token).is_some() {
                    return Ok((port as u16, token.to_string()));
                }
            }
        }
        if Instant::now() > deadline {
            let tail = std::fs::read_to_string(agent_home().join("server.log")).unwrap_or_default();
            bail!(
                "the agent did not start: {}",
                tail.lines().rev().take(3).collect::<Vec<_>>().join(" | ")
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

// ---- over SSH ----------------------------------------------------------------------

fn ssh(host: &str, script: &str, stdin: Option<&[u8]>) -> Result<std::process::Output> {
    let mut child = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            host,
            "bash",
            "-lc",
        ])
        .arg(format!("'{}'", script.replace('\'', r"'\''")))
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("ssh to host")?;
    if let Some(bytes) = stdin {
        use std::io::Write as _;
        child.stdin.take().unwrap().write_all(bytes)?;
    }
    Ok(child.wait_with_output()?)
}

/// Whether the host answers over SSH (Err when it does not), and has a usable Node.
pub fn remote_node(host: &str) -> Result<bool> {
    let out = ssh(host, "node --version 2>/dev/null", None)?;
    if out.status.code() == Some(255) {
        bail!("{host}: SSH could not connect");
    }
    Ok(node_ok(&String::from_utf8_lossy(&out.stdout)))
}

/// Installs (when its sources changed) and starts the agent on `host`; its port and token there.
pub fn ensure_remote(host: &str) -> Result<(u16, String)> {
    if !remote_node(host)? {
        bail!("{host}: Bench's agent needs Node.js 22.18 or newer there");
    }
    let (src, hash) = source()?;
    let installed = ssh(host, "cat ~/.bench/app/.build 2>/dev/null", None)?;
    if String::from_utf8_lossy(&installed.stdout).trim() != hash {
        let tar = Command::new("tar")
            .arg("-C")
            .arg(&src)
            .args(["-czf", "-"])
            .args(PARTS)
            .env("COPYFILE_DISABLE", "1")
            .output()
            .context("pack the agent")?;
        let script = format!(
            "set -e; mkdir -p ~/.bench/app; cd ~/.bench/app; rm -rf src scripts; tar -xzf - 2>/dev/null; \
             if ! cmp -s package-lock.json .lock-installed || [ ! -d node_modules ]; then npm ci --omit=dev --no-audit --no-fund >/dev/null; cp package-lock.json .lock-installed; fi; \
             echo {hash} > .build"
        );
        let out = ssh(host, &script, Some(&tar.stdout))?;
        if !out.status.success() {
            bail!(
                "{host}: installing the agent failed: {}",
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .last()
                    .unwrap_or("")
            );
        }
    }
    let read_state = || -> Option<Value> {
        let out = ssh(host, "cat ~/.bench/server.json 2>/dev/null", None).ok()?;
        serde_json::from_slice(&out.stdout).ok()
    };
    // A server of this build that is still alive is reused.
    if let Some(v) = read_state() {
        let alive = v["pid"].as_u64().is_some_and(|pid| {
            ssh(host, &format!("kill -0 {pid} 2>/dev/null"), None).is_ok_and(|o| o.status.success())
        });
        if alive && v["build"].as_str() == Some(hash.as_str()) {
            if let (Some(port), Some(token)) = (v["port"].as_u64(), v["token"].as_str()) {
                return Ok((port as u16, token.to_string()));
            }
        }
        if alive {
            let _ = ssh(
                host,
                &format!("kill {} 2>/dev/null; sleep 1", v["pid"]),
                None,
            );
        }
    }
    let script = format!(
        "cd ~/.bench/app || exit 1; rm -f ~/.bench/server.json; \
         BENCH_AGENT_BUILD={hash} nohup setsid node src/server.ts > ~/.bench/server.log 2>&1 < /dev/null & \
         for i in $(seq 1 60); do [ -s ~/.bench/server.json ] && grep -q BENCH_AGENT_READY ~/.bench/server.log && {{ cat ~/.bench/server.json; exit 0; }}; sleep 1; done; tail -3 ~/.bench/server.log; exit 1"
    );
    let out = ssh(host, &script, None)?;
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        anyhow!(
            "{host}: the agent did not start: {}",
            String::from_utf8_lossy(&out.stdout).trim()
        )
    })?;
    match (v["port"].as_u64(), v["token"].as_str()) {
        (Some(port), Some(token)) => Ok((port as u16, token.to_string())),
        _ => bail!("{host}: the agent wrote no port"),
    }
}

/// The agent runs under the Bench that started it, so macOS's Local Network privacy refuses its
/// connections to the local network (EHOSTUNREACH) until Bench has been allowed, and a child
/// process never brings up the prompt itself. When the agent reports such an error
/// ("192.168.1.100:10240: EHOSTUNREACH …"), Bench connects to the same address once, which
/// makes macOS ask "Allow Bench to find devices on your local network?". True when it did.
pub fn ask_local_network(error: &str) -> bool {
    let Some(i) = error.find(": EHOSTUNREACH") else {
        return false;
    };
    let addr = error[..i].rsplit([' ', '"']).next().unwrap_or_default();
    let Ok(addr) = addr.parse::<std::net::SocketAddr>() else {
        return false;
    };
    std::thread::spawn(move || {
        let _ = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(3));
    });
    true
}

#[cfg(test)]
mod tests {
    use super::ask_local_network;

    #[test]
    fn local_network_error_is_recognized() {
        assert!(ask_local_network(
            "502: 127.0.0.1:9: EHOSTUNREACH (allow Bench in System Settings)"
        ));
        assert!(!ask_local_network("502: 127.0.0.1:9: ECONNREFUSED"));
        assert!(!ask_local_network("502: fetch failed"));
    }
}
