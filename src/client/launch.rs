//! The agent Bench talks to: bench-agent (`agent/` in this repo), a server on Pi Durable. The
//! same on this Mac and on a server over SSH: Bench installs its sources there, runs it as the
//! machine's service (`service.rs`), then speaks its protocol (JSON-RPC over `/api/ws?token=`,
//! REST under `/api/` with `X-Bench-Token`).
//!
//! * Its home is `~/.bench` on each machine: `bench.sqlite`, `pi.sqlite`, `agents/`, `projects/`,
//!   `extensions/`, and `app/` — the installed server, unpacked from the sources this app carries
//!   (`Contents/Resources/agent`, or this repo's `agent/` for a dev build) with their hash in
//!   `app/.build`; `npm ci` runs there when `package-lock.json` changed. Node 22.18+ is needed.
//! * The server writes its port, token, pid and build to `~/.bench/server.json`. An agent of
//!   other sources is restarted at once: its runs are durable, one it was in resumes.
//! * A copy of the home (`BENCH_HOME`, to try Bench on other data) gets no service: its agent is
//!   started by hand (`BENCH_HOME=… node agent/src/server.ts`) and Bench only connects to it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use serde_json::Value;

use crate::app::model::Target;

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
const PARTS: [&str; 3] = ["src", "package.json", "package-lock.json"];

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

/// The build of the agent this app carries (what `ensure` installs where it runs).
pub fn build() -> Option<String> {
    source().ok().map(|(_, hash)| hash)
}

fn source() -> Result<(PathBuf, String)> {
    let src = bundled_source().ok_or_else(|| anyhow!("this build carries no agent sources"))?;
    let hash = source_hash(&src);
    Ok((src, hash))
}

// ---- any machine -----------------------------------------------------------------

/// The lines every script on a machine begins with: the agent's home (`$H`) and a Node
/// (`$NODE`), found where Homebrew and installers put it even when the login shell does not say.
const PRELUDE: &str = r#"H="$HOME/.bench"
export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
NODE="$(command -v node)"
"#;

/// Unpacks the sources (a tar on stdin) into `$H/app`, installs its packages when the lock
/// changed, and records the build. `npm ci` empties `node_modules` before it downloads, and the
/// agent keeps loading from there, so a network failure would leave it running without packages:
/// they are installed aside and put in place in one step. A failed install changes nothing and is
/// tried again (`.lock-installed`).
const INSTALL: &str = r#"set -e
mkdir -p "$H/app"; cd "$H/app"; rm -rf src scripts; tar -xzf - 2>/dev/null
if ! cmp -s package-lock.json .lock-installed || [ ! -d node_modules ]; then
	rm -rf .install; mkdir .install; cp package.json package-lock.json .install/
	(cd .install && PATH="$(dirname "$NODE"):$PATH" npm ci --omit=dev --no-audit --no-fund >/dev/null)
	rm -rf node_modules; mv .install/node_modules node_modules; rm -rf .install
	cp package-lock.json .lock-installed
fi
echo @BUILD@ > .build
"#;

/// Runs a script on `target`'s machine in a login shell, after `PRELUDE`: here with a clean
/// environment (the service written from it must not depend on how Bench was opened), there over
/// SSH.
fn sh(target: &Target, script: &str, stdin: Option<&[u8]>) -> Result<std::process::Output> {
    let script = format!("{PRELUDE}{script}");
    let mut cmd = match target {
        Target::Local => {
            let mut cmd = Command::new("/usr/bin/env");
            cmd.arg("-i");
            for var in ["HOME", "USER", "LOGNAME"] {
                cmd.arg(format!("{var}={}", std::env::var(var).unwrap_or_default()));
            }
            cmd.args(["bash", "-lc", &script]);
            cmd
        }
        Target::Ssh(host) => {
            let mut cmd = Command::new("ssh");
            cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=8", host, "bash", "-lc"])
                .arg(format!("'{}'", script.replace('\'', r"'\''")));
            cmd
        }
    };
    let mut child = cmd
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("run a script for the agent")?;
    if let Some(bytes) = stdin {
        use std::io::Write as _;
        child.stdin.take().unwrap().write_all(bytes)?;
    }
    let out = child.wait_with_output()?;
    if matches!(target, Target::Ssh(_)) && out.status.code() == Some(255) {
        bail!("{}: SSH could not connect", machine(target));
    }
    Ok(out)
}

fn machine(target: &Target) -> String {
    match target {
        Target::Local => "this Mac".into(),
        Target::Ssh(host) => host.clone(),
    }
}

/// The last line a failed script said.
fn said(out: &std::process::Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr);
    err.lines().last().unwrap_or("").trim().to_string()
}

/// Whether the machine answers (Err when it does not), and has a Node the agent can run on.
pub fn node_ready(target: &Target) -> Result<bool> {
    let out = sh(target, r#""$NODE" --version 2>/dev/null"#, None)?;
    Ok(node_ok(&String::from_utf8_lossy(&out.stdout)))
}

/// Puts the sources this app carries into `~/.bench/app` there, when its `.build` differs.
fn install(target: &Target, src: &Path, hash: &str) -> Result<()> {
    let installed = sh(target, r#"cat "$H/app/.build" 2>/dev/null"#, None)?;
    if String::from_utf8_lossy(&installed.stdout).trim() == hash {
        return Ok(());
    }
    let tar = Command::new("tar")
        .arg("-C")
        .arg(src)
        .args(["-czf", "-"])
        .args(PARTS)
        .env("COPYFILE_DISABLE", "1")
        .output()
        .context("pack the agent")?;
    let out = sh(target, &INSTALL.replace("@BUILD@", hash), Some(&tar.stdout))?;
    if !out.status.success() {
        bail!("{}: installing the agent failed: {}", machine(target), said(&out));
    }
    Ok(())
}

/// A copy of the home (`BENCH_HOME`): its agent is the tester's, never a service.
fn is_copy() -> bool {
    std::env::var_os("BENCH_HOME").is_some()
}

/// The running agent of a copy of the home: its port and token.
fn attach_copy() -> Result<(u16, String)> {
    let home = agent_home();
    let v: Value = std::fs::read(home.join("server.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if let (Some(port), Some(token)) = (v["port"].as_u64(), v["token"].as_str()) {
        let answers = ureq::get(&format!("http://127.0.0.1:{port}/api/status"))
            .set("X-Bench-Token", token)
            .timeout(Duration::from_secs(3))
            .call()
            .is_ok();
        if answers {
            return Ok((port as u16, token.to_string()));
        }
    }
    bail!(
        "this copy's agent is not running: start it with BENCH_HOME={} node agent/src/server.ts",
        home.display()
    )
}

/// Installs (when its sources changed) and brings up the agent on `target`'s machine as its
/// service; its port and token there.
pub fn ensure(target: &Target) -> Result<(u16, String)> {
    if matches!(target, Target::Local) && is_copy() {
        return attach_copy();
    }
    if !node_ready(target)? {
        bail!(
            "{}: Bench's agent needs Node.js 22.18 or newer there",
            machine(target)
        );
    }
    let (src, hash) = source()?;
    install(target, &src, &hash)?;
    let out = sh(target, &super::service::start(&hash), None)?;
    if !out.status.success() {
        bail!("{}: {}", machine(target), said(&out));
    }
    let v: Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| anyhow!("{}: the agent wrote no server.json", machine(target)))?;
    match (v["port"].as_u64(), v["token"].as_str()) {
        (Some(port), Some(token)) => Ok((port as u16, token.to_string())),
        _ => bail!("{}: the agent wrote no port", machine(target)),
    }
}

/// Restarts the agent's service on `target`'s machine (its unfinished runs continue after the
/// start).
pub fn restart(target: &Target) -> Result<()> {
    if matches!(target, Target::Local) && is_copy() {
        bail!("this copy's agent was started by hand: restart it the same way");
    }
    let out = sh(target, &super::service::restart(), None)?;
    if !out.status.success() {
        bail!("{}: restarting the agent failed: {}", machine(target), said(&out));
    }
    Ok(())
}

/// macOS's Local Network privacy refuses Bench's connections to the local network (EHOSTUNREACH)
/// until Bench has been allowed. When a request reports such an error
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
