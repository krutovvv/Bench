//! What this Bench has. Every Bench starts as a copy of the reference one with everything on;
//! switch off what its person doesn't use: the mode's button and shortcut go, its code stays and
//! still builds, so switching it back on is one word.

/// SSH servers with their terminals (⌘2, the terminal icon at the end of the header).
pub const SSH: bool = true;
/// The mini IDE beside a chat or a terminal: file tree, editor, tabs (⌘B, ⌘P, "Open files").
pub const IDE: bool = true;
/// The Tools mode of everyday utilities (⌘3, the wrench in the status bar).
pub const TOOLS: bool = true;
