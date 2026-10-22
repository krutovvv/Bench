//! One way to reach a server of `~/.ssh/config`: `/usr/bin/ssh` through a shared connection.
//! Everything that talks to a server — the terminal, the IDE's files, the ssh-mcp server the
//! agent uses — goes through the same ControlMaster socket in `~/.ssh/cm/` (one login per
//! server, kept 10 idle minutes), so only the first of them logs in. The user's ssh config is
//! never changed; the gateway connection (`connection.rs`) keeps its own tunnel.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

/// Options for an interactive `ssh` (the terminal): reuse the shared connection, or become it.
/// A master started this way keeps running detached (its stdio on /dev/null) after the
/// terminal closes, for the IDE and the agent.
pub fn share_args() -> Vec<String> {
    vec![
        "-o".into(),
        "ControlMaster=auto".into(),
        "-o".into(),
        format!("ControlPath={}/%C", control_dir().display()),
        "-o".into(),
        "ControlPersist=600".into(),
    ]
}

pub fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

pub fn control_dir() -> PathBuf {
    // Short: a Unix socket path must stay under 104 bytes.
    let dir = home().join(".ssh/cm");
    if !dir.exists() {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    dir
}

/// Single-quote for a POSIX shell.
pub fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn ssh(master: &str) -> Command {
    let mut c = Command::new("/usr/bin/ssh");
    c.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ServerAliveInterval=15",
    ])
    .arg("-o")
    .arg(format!("ControlMaster={master}"))
    .arg("-o")
    .arg(format!("ControlPath={}/%C", control_dir().display()));
    c
}

/// A live shared connection, started if needed. The master is detached with its stdio on
/// /dev/null and a file: one that inherited our pipes would keep them open forever.
pub fn ensure_master(alias: &str) -> Result<()> {
    let alive = ssh("no")
        .args(["-O", "check", alias])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success();
    if alive {
        return Ok(());
    }
    let log = control_dir().join("last-error.log");
    let status = ssh("yes")
        .args(["-o", "ControlPersist=600", "-N", "-f", alias])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&log)?)
        .status()?;
    if !status.success() {
        let err = std::fs::read_to_string(&log).unwrap_or_default();
        bail!("ssh {alias}: {}", err.trim());
    }
    Ok(())
}

/// Run `remote` on the server; stdout on success, stderr as the error otherwise.
pub fn run(
    alias: &str,
    remote: &str,
    stdin: Option<Vec<u8>>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    ensure_master(alias)?;
    let mut child = ssh("no")
        .arg("-T")
        .arg(alias)
        .arg("--")
        .arg(remote)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let (Some(data), Some(mut w)) = (stdin, child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = w.write_all(&data);
        });
    }
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = so.read_to_end(&mut b);
        b
    });
    let err = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = se.read_to_end(&mut b);
        b
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            bail!("ssh {alias}: timed out after {}s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    if !status.success() {
        let msg = String::from_utf8_lossy(&err).trim().to_string();
        bail!(
            "{}",
            if msg.is_empty() {
                format!("exit code {}", status.code().unwrap_or(-1))
            } else {
                msg
            }
        );
    }
    Ok(out)
}
