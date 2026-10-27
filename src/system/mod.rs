//! What Bench asks of the machine: SSH and the servers of `~/.ssh/config`, system notifications.

#[cfg(target_os = "macos")]
pub mod notify;
#[cfg(not(target_os = "macos"))]
#[path = "notify_other.rs"]
pub mod notify;
pub mod ssh;
pub mod sshconfig;
