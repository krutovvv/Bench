//! Servers live in `~/.ssh/config` — the system's own list, which ssh, scp, Zed and every other
//! tool read too. Nothing is kept in the app. The app's own fields are comments inside the Host
//! block, which ssh ignores: `# folder: <name>` (the list's group) and `# path: <dir>` (the
//! server's default directory: files and Zed open there; the terminal still starts at home). Every write keeps a backup and the file's 0600 mode.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context as _, Result, bail};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Server {
    /// The `Host` alias: what `ssh <alias>` takes.
    pub alias: String,
    pub hostname: String,
    /// The private key (`IdentityFile`, the first one if there are several); empty = ssh's
    /// defaults and ssh-agent. Passwords are never stored.
    pub identity: String,
    pub user: String,
    pub port: String,
    pub folder: String,
    /// Default directory on the server; empty = the login directory.
    pub path: String,
    /// The block names several aliases (`Host a b`): editable only by hand.
    pub shared: bool,
}

pub fn path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".ssh/config")
}

/// `(key, value)` of a config line, key lowercased; `Key value` and `Key=value` both work.
fn key_value(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    let (k, v) = t
        .split_once(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or((t, ""));
    Some((
        k.to_lowercase(),
        v.trim_start_matches(|c: char| c.is_whitespace() || c == '=')
            .trim()
            .to_string(),
    ))
}

/// One of the app's comment fields (`# folder: …`, `# path: …`) with its key lowercased.
fn meta_of(line: &str) -> Option<(String, String)> {
    let t = line.trim().strip_prefix('#')?.trim();
    let (k, v) = t.split_once(':')?;
    let k = k.trim().to_lowercase();
    (k == "folder" || k == "path").then(|| (k, v.trim().to_string()))
}

/// Line ranges of the Host blocks: (first line, end line exclusive, aliases).
fn blocks(lines: &[&str]) -> Vec<(usize, usize, Vec<String>)> {
    let mut out: Vec<(usize, usize, Vec<String>)> = vec![];
    for (i, line) in lines.iter().enumerate() {
        if let Some((k, v)) = key_value(line) {
            if k == "host" || k == "match" {
                if let Some(last) = out.last_mut() {
                    last.1 = i;
                }
                let names = if k == "host" {
                    v.split_whitespace().map(str::to_string).collect()
                } else {
                    vec![]
                };
                out.push((i, lines.len(), names));
            }
        }
    }
    // Trailing blank lines belong to no block.
    for b in out.iter_mut() {
        while b.1 > b.0 + 1 && lines[b.1 - 1].trim().is_empty() {
            b.1 -= 1;
        }
    }
    out
}

/// Every concrete host (no wildcards) of `~/.ssh/config`, in file order.
pub fn load() -> Vec<Server> {
    load_from(&path())
}

fn load_from(file: &std::path::Path) -> Vec<Server> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Server> = vec![];
    for (start, end, names) in blocks(&lines) {
        let concrete: Vec<&String> = names
            .iter()
            .filter(|h| !h.contains(['*', '?', '!']))
            .collect();
        let mut s = Server {
            shared: names.len() > 1,
            ..Default::default()
        };
        for line in &lines[start + 1..end] {
            match meta_of(line) {
                Some((k, v)) if k == "folder" => s.folder = v,
                Some((_, v)) => s.path = v,
                None => {}
            }
            match key_value(line) {
                Some((k, v)) if k == "hostname" => s.hostname = v,
                Some((k, v)) if k == "user" => s.user = v,
                Some((k, v)) if k == "port" => s.port = v,
                Some((k, v)) if k == "identityfile" && s.identity.is_empty() => {
                    s.identity = v.trim_matches('"').to_string()
                }
                _ => {}
            }
        }
        for alias in concrete {
            if !out.iter().any(|o| &o.alias == alias) {
                out.push(Server {
                    alias: alias.clone(),
                    ..s.clone()
                });
            }
        }
    }
    out
}

/// The indentation the file already uses for options (first indented option line), else 4.
fn indent_of(lines: &[&str]) -> String {
    lines
        .iter()
        .filter(|l| key_value(l).is_some_and(|(k, _)| k != "host" && k != "match"))
        .find_map(|l| {
            let n = l.len() - l.trim_start().len();
            (n > 0).then(|| l[..n].to_string())
        })
        .unwrap_or_else(|| "    ".into())
}

fn block_text(s: &Server, indent: &str) -> Vec<String> {
    let mut v = vec![format!("Host {}", s.alias)];
    for (key, value) in [
        ("HostName", &s.hostname),
        ("User", &s.user),
        ("Port", &s.port),
    ] {
        if !value.trim().is_empty() {
            v.push(format!("{indent}{key} {}", value.trim()));
        }
    }
    let key = s.identity.trim();
    if !key.is_empty() {
        // Quoted when it has spaces (ssh accepts "…").
        if key.contains(' ') {
            v.push(format!("{indent}IdentityFile \"{key}\""));
        } else {
            v.push(format!("{indent}IdentityFile {key}"));
        }
    }
    if !s.folder.trim().is_empty() {
        v.push(format!("{indent}# folder: {}", s.folder.trim()));
    }
    if !s.path.trim().is_empty() {
        v.push(format!("{indent}# path: {}", s.path.trim()));
    }
    v
}

fn check(s: &Server) -> Result<()> {
    let ok = |v: &str| !v.chars().any(|c| c.is_whitespace() || c == '#' || c == '=');
    if s.alias.trim().is_empty() {
        bail!("the name is empty");
    }
    if !ok(&s.alias) || s.alias.contains(['*', '?', '!']) {
        bail!("the name can't contain spaces, # = * ? !");
    }
    if !ok(&s.hostname) || !ok(&s.user) {
        bail!("host and user can't contain spaces, # or =");
    }
    if !s.port.trim().is_empty() && s.port.trim().parse::<u16>().is_err() {
        bail!("the port must be a number");
    }
    if s.identity.contains('"') {
        bail!("the key path can't contain quotes");
    }
    Ok(())
}

/// A key typed into the form must exist; one already in the config is left as it is (it may
/// use ssh's `%` tokens, or live where only ssh finds it).
fn check_key(s: &Server, before: Option<&Server>) -> Result<()> {
    let key = s.identity.trim();
    let unchanged = before.is_some_and(|b| b.identity.trim() == key);
    if !key.is_empty() && !unchanged && !key.contains('%') && !expand_home(key).is_file() {
        bail!("no key file at {key}");
    }
    Ok(())
}

/// `~/x` → `$HOME/x`.
pub fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => path().parent().unwrap().parent().unwrap().join(rest),
        None => PathBuf::from(p),
    }
}

/// `$HOME/x` → `~/x` (what ssh configs usually say).
pub fn tilde(p: &std::path::Path) -> String {
    let home = path().parent().unwrap().parent().unwrap().to_path_buf();
    match p.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

/// Add a new Host block (`old` = None) or rewrite the server's own lines of an existing one;
/// other options in that block (IdentityFile, ProxyJump, …) stay as they are.
pub fn save(old: Option<&str>, s: &Server) -> Result<()> {
    save_in(&path(), old, s)
}

fn save_in(file: &std::path::Path, old: Option<&str>, s: &Server) -> Result<()> {
    check(s)?;
    let before = old.and_then(|o| load_from(file).into_iter().find(|x| x.alias == o));
    check_key(s, before.as_ref())?;
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let bl = blocks(&lines);
    let find = |alias: &str| bl.iter().find(|b| b.2.iter().any(|n| n == alias));
    if old != Some(s.alias.as_str()) && find(&s.alias).is_some() {
        bail!("“{}” is already in ~/.ssh/config", s.alias);
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    match old.and_then(find) {
        None => {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.extend(block_text(s, &indent_of(&lines)));
        }
        Some((start, end, names)) => {
            if names.len() > 1 {
                bail!("this Host line names several hosts; edit ~/.ssh/config by hand");
            }
            // The form owns these lines (and the first IdentityFile); everything else stays.
            let mut first_key = true;
            let keep: Vec<String> = lines[start + 1..*end]
                .iter()
                .filter(|l| {
                    if matches!(key_value(l), Some((k, _)) if k == "identityfile") {
                        return !std::mem::replace(&mut first_key, false);
                    }
                    meta_of(l).is_none()
                        && !matches!(key_value(l), Some((k, _)) if k == "hostname" || k == "user" || k == "port")
                })
                .map(|l| l.to_string())
                .collect();
            let mut block = block_text(s, &indent_of(&lines[*start..*end]));
            block.extend(keep);
            out.splice(*start..*end, block);
        }
    }
    write(file, &out)
}

pub fn delete(alias: &str) -> Result<()> {
    delete_in(&path(), alias)
}

fn delete_in(file: &std::path::Path, alias: &str) -> Result<()> {
    let text = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = text.lines().collect();
    let Some((start, end, names)) = blocks(&lines)
        .into_iter()
        .find(|b| b.2.iter().any(|n| n == alias))
    else {
        bail!("“{alias}” is not in ~/.ssh/config");
    };
    if names.len() > 1 {
        bail!("this Host line names several hosts; edit ~/.ssh/config by hand");
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    let mut end = end;
    // Take one blank separator line with it.
    if out.get(end).is_some_and(|l| l.trim().is_empty()) {
        end += 1;
    }
    out.drain(start..end);
    write(file, &out)
}

fn write(p: &std::path::Path, lines: &[String]) -> Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::create_dir_all(p.parent().unwrap())?;
    if p.exists() {
        std::fs::copy(&p, p.with_file_name("config.bak-bench")).context("back up ~/.ssh/config")?;
    }
    let tmp = p.with_file_name(".config.bench.tmp");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let mut text = lines[..end].join("\n");
    text.push('\n');
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, &p).context("write ~/.ssh/config")?;
    Ok(())
}

/// `user@hostname` as ssh resolves the alias (`ssh -G`: reads config only, no network).
pub fn resolve(alias: &str) -> Option<String> {
    let out = Command::new("/usr/bin/ssh")
        .arg("-G")
        .arg(alias)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let (mut user, mut host) = (String::new(), String::new());
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        match line.split_once(' ') {
            Some(("user", v)) => user = v.to_string(),
            Some(("hostname", v)) => host = v.to_string(),
            _ => {}
        }
    }
    Some(if user.is_empty() {
        host
    } else {
        format!("{user}@{host}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A config file of its own in a fresh temporary folder (never `~/.ssh/config`).
    fn scratch(name: &str, text: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("herm-sshconfig-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config");
        std::fs::write(&file, text).unwrap();
        file
    }

    const SAMPLE: &str = "Include ~/.orbstack/ssh/config\n\nHost web\n  HostName 10.0.0.5\n  User deploy\n  IdentityFile ~/.ssh/web\n  IdentityFile ~/.ssh/web-old\n  # folder: Prod\n  # path: /srv/www\n\nHost a b\n  User x\n";

    #[test]
    fn reads_fields_and_comments() {
        let f = scratch("read", SAMPLE);
        let all = load_from(&f);
        let web = all.iter().find(|s| s.alias == "web").unwrap();
        assert_eq!(
            (web.hostname.as_str(), web.user.as_str()),
            ("10.0.0.5", "deploy")
        );
        assert_eq!(web.identity, "~/.ssh/web", "the first IdentityFile");
        assert_eq!(
            (web.folder.as_str(), web.path.as_str()),
            ("Prod", "/srv/www")
        );
        assert!(all.iter().find(|s| s.alias == "a").unwrap().shared);
    }

    #[test]
    fn add_edit_delete_round_trip() {
        let f = scratch("edit", SAMPLE);
        let key = f.parent().unwrap().join("key");
        std::fs::write(&key, "").unwrap();
        let new = Server {
            alias: "db".into(),
            hostname: "10.0.0.9".into(),
            port: "2222".into(),
            identity: key.display().to_string(),
            folder: "Data".into(),
            ..Default::default()
        };
        save_in(&f, None, &new).unwrap();
        assert!(save_in(&f, None, &new).is_err(), "a second db is refused");
        // Edit web: other options of the block (the second IdentityFile) stay.
        let web = load_from(&f)
            .into_iter()
            .find(|s| s.alias == "web")
            .unwrap();
        save_in(
            &f,
            Some("web"),
            &Server {
                path: "/var/www".into(),
                ..web
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(text.contains("# path: /var/www") && !text.contains("/srv/www"));
        assert!(text.contains("IdentityFile ~/.ssh/web-old"));
        assert!(
            text.contains("  HostName 10.0.0.5"),
            "the file's own indent"
        );
        // A missing key file and a shared Host line are refused.
        let bad = Server {
            identity: "/nope/key".into(),
            ..new.clone()
        };
        assert!(save_in(&f, Some("db"), &bad).is_err());
        let a = load_from(&f).into_iter().find(|s| s.alias == "a").unwrap();
        assert!(save_in(&f, Some("a"), &a).is_err());
        delete_in(&f, "db").unwrap();
        assert!(load_from(&f).iter().all(|s| s.alias != "db"));
        // Backup kept, mode 0600.
        assert!(f.with_file_name("config.bak-bench").exists());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = std::fs::remove_dir_all(f.parent().unwrap());
    }

    #[test]
    fn blocks_and_folders() {
        let text = "Host a\n  HostName 1.2.3.4\n  # folder: Prod\n\nHost b c\n  User x\nHost *\n  ServerAliveInterval 5\n";
        let lines: Vec<&str> = text.lines().collect();
        let b = blocks(&lines);
        assert_eq!(b.len(), 3);
        assert_eq!((b[0].0, b[0].1), (0, 3));
        assert_eq!(
            meta_of("  # folder: Prod"),
            Some(("folder".into(), "Prod".into()))
        );
        assert_eq!(
            meta_of("# Path: /var/www"),
            Some(("path".into(), "/var/www".into()))
        );
        assert_eq!(
            key_value("  HostName=1.2.3.4"),
            Some(("hostname".into(), "1.2.3.4".into()))
        );
    }
}
