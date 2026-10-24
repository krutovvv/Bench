//! What this Bench has. Every Bench starts as a copy of the user's with everything on; hide the
//! built-in tools its person doesn't use: the sidebar group and the shortcut go, the code stays and
//! still builds, so bringing one back is one word.

/// Servers: `~/.ssh/config` hosts and this Mac's terminal as threads (⌘2).
pub const SSH: bool = true;
/// The mini IDE beside a thread: file tree, editor, tabs (⌘B, ⌘P, "Open files").
pub const IDE: bool = true;
/// Tools: everyday utilities, one thread per tool (⌘3).
pub const TOOLS: bool = true;
/// Tasks: the task lists as threads, kept in `~/.bench/bench.sqlite` (⌘4).
pub const TASKS: bool = true;
