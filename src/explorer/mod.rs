//! The Explorer: a project seen as an editor sees it — a panel of the left dock (its tree), its
//! documents in the center with tabs, the thread in the right dock (`docs/explorer.md`,
//! `src/workspace/mod.rs`). The tree has the project's folder (on its
//! machine), what its agent reads as text (the soul, prompts, skills), its tools (MCP and
//! language servers) and its schedule.
//!
//! ```text
//! explorer/mod.rs   the state: the project shown, folds, listings, open documents, tabs
//! explorer/tree/    the tree: drawn, its parts, its menus, new / rename / delete
//! explorer/pane/    the middle: documents, tabs, a file's editor, kept between starts
//! explorer/search/  ⌘P a file by name, ⌘⇧F a text in the folder
//! explorer/specs/   the project's specs: their tree by parent, a spec as a tab with its links
//! explorer/statusbar.rs  its status bar: the caret, the thread beside, the terminal
//! explorer/code/    language servers (problems, hover, definition), Problems, git, languages
//! ```


pub mod code;
pub mod pane;
pub mod search;
pub mod specs;
mod statusbar;
pub mod tree;

use std::collections::{HashMap, HashSet};

use gpui_kit::component::input::InputState;
use gpui_kit::*;

pub use pane::doc::{Doc, DocKey, FileRef};

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
    /// Every project's open nodes, the shown one's as it was when another was shown.
    pub folds: HashMap<String, HashSet<String>>,
    /// The kept tabs and folds were read back (`persistence.rs`): changes are written from now.
    pub restored: bool,
    /// Folders listed so far, by their path (a project's machine's or the agent's).
    pub listings: HashMap<String, Vec<Entry>>,
    /// What git ignores, per repository the tree has shown (`/api/git/ignored`): the root, then
    /// the absolute paths of the ignored files and folders. A row on one of these is dimmed —
    /// ignored is noise to see past, as Zed shows it.
    pub ignored: Vec<(String, HashSet<String>)>,
    /// Folders whose ignored list was asked for ("": not a repository is an answer too).
    pub ignored_asked: HashSet<String>,
    /// Open documents, the tabs of the middle in order, and the one shown there.
    pub docs: HashMap<DocKey, Doc>,
    pub tabs: Vec<DocKey>,
    pub active: Option<DocKey>,
    pub naming: Option<Naming>,
    /// A comment being typed on lines of a file (⌘⇧L, `pane/comment.rs`).
    pub comment: Option<pane::comment::Commenting>,
    pub quick: Option<Quick>,
    /// Each project's Problems as last read (None: being read): (path, its problems).
    pub problems: HashMap<String, Option<Vec<(String, Vec<lsp_types::Diagnostic>)>>>,
    pub find: Option<Find>,
    /// Each project's specs as last read (None: being read), `specs/`; the specs' texts open
    /// in tabs, by path (None: being read).
    pub specs: HashMap<String, Option<specs::Specs>>,
    pub spec_texts: HashMap<String, Option<String>>,
    /// The left dock lists the project's threads in place of its tree (Files | Threads at its
    /// top): one clicked is the thread beside the document.
    pub threads: bool,
}

impl Explorer {
    /// The open project's tabs, in order: a project shows its own documents only; another's
    /// stay open for when it is back.
    pub fn project_tabs(&self) -> Vec<DocKey> {
        self.tabs.iter().filter(|k| self.owns(k)).cloned().collect()
    }

    /// The document belongs to the open project.
    pub fn owns(&self, key: &DocKey) -> bool {
        key.project() == self.project
    }
}
