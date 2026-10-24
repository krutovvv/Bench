//! Where the app keeps its own small files (preferences, last place, logs).

use std::path::PathBuf;

pub fn state_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/__CRATE__")
}

/// A Finder-launched app has no stderr: errors go to a log file here.
pub fn log_error(what: &str, err: &str) {
    use std::io::Write as _;
    let _ = std::fs::create_dir_all(state_dir());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir().join("errors.log"))
    {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let _ = writeln!(f, "{ts} {what}: {err}");
    }
}
