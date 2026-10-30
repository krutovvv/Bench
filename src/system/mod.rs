//! What Bench asks of the machine: SSH and the servers of `~/.ssh/config`, system notifications.

#[cfg(target_os = "macos")]
pub mod notify;
#[cfg(not(target_os = "macos"))]
#[path = "notify_other.rs"]
pub mod notify;
pub mod ssh;
pub mod sshconfig;

/// Running from inside an .app bundle: the person's Bench, not a bare build (the only place
/// notifications can work, and the only one that makes a sound).
pub fn bundled() -> bool {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}
