//! The Explorer: a project seen as an editor sees it — a panel of the left dock (its tree), its
//! documents in the center with tabs, a file kept in the right dock (`docs/explorer.md`,
//! `src/app/workspace.rs`). The tree has the project's folder (on its
//! machine), what its agent reads as text (the soul, prompts, skills), its tools (MCP and
//! language servers) and its schedule.
//!
//! ```text
//! explorer/mod.rs   the state: the project shown, folds, listings, open documents, tabs
//! explorer/doc.rs   documents: a file (an editor of its own), or a part of the environment
//! explorer/ops.rs   the tree's work: listing a folder, new / rename / delete, ⌘P, ⌘⇧F
//! explorer/tree.rs  the sidebar's tree
//! explorer/view.rs  a document in the middle and beside, the tabs, ⌘P's list
//! ```

pub mod doc;
pub mod ops;
pub mod tree;
pub mod view;

use std::collections::{HashMap, HashSet};

use gpui_kit::component::input::InputState;
use gpui_kit::*;

pub use doc::{Doc, DocKey, FileRef};

/// A folder's entry, as listed: its name, its full path, whether it is a folder.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub dir: bool,
}

/// A name being typed in the tree, where it will go; Enter applies it, Esc drops it.
pub struct Naming {
    pub what: Named,
    pub input: Entity<InputState>,
}

/// What a name is typed for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Named {
    /// A new file, or folder, in this folder.
    File(FileRef),
    Folder(FileRef),
    /// This entry's new name.
    Rename(FileRef),
    /// The project's new prompt, or skill.
    Prompt,
    Skill,
}

/// ⌘P: every file of the project's folder (listed once it opens), the query, the line chosen.
pub struct Quick {
    pub input: Entity<InputState>,
    pub files: Option<Vec<String>>,
    pub sel: usize,
}

/// ⌘⇧F: the text looked for, and the lines that have it (path relative to the folder).
pub struct Find {
    pub input: Entity<InputState>,
    pub hits: Option<Vec<(String, usize, String)>>,
    pub asked: String,
}

#[derive(Default)]
pub struct Explorer {
    /// The project shown (as `ProjectEnv` names it: "general" is this Mac's Host).
    pub project: String,
    /// Open nodes of the tree (`tree.rs` names them).
    pub open: HashSet<String>,
    /// Folders listed so far, by their path (a project's machine's or the agent's).
    pub listings: HashMap<String, Vec<Entry>>,
    /// Open documents, the tabs of the middle in order, and the one shown there.
    pub docs: HashMap<DocKey, Doc>,
    pub tabs: Vec<DocKey>,
    pub active: Option<DocKey>,
    /// The file kept in the right dock, and the dock's width once dragged.
    pub beside: Option<FileRef>,
    pub beside_w: Option<Pixels>,
    pub naming: Option<Naming>,
    /// A delete or a close asked once: the second click does it.
    pub armed: Option<String>,
    pub quick: Option<Quick>,
    pub find: Option<Find>,
}
