//! The window's threads beyond the ones on screen: every agent's list (the sidebar's rows), the
//! journals of threads seen in this run, the drafts and the code references waiting in their
//! composers, the thread to reopen, and a new chat being made.

use std::collections::HashMap;

use crate::app::model::ThreadRow;
use crate::thread::attachments::Reference;

pub struct ThreadStore {
    /// Every agent's threads, as each last listed them.
    pub list: Vec<ThreadRow>,
    /// Each thread opened in this run, as last shown: switching back shows it at once.
    pub cache: HashMap<String, ::thread::Thread>,
    /// Unsent text per thread, kept on disk (`drafts.rs`).
    pub drafts: HashMap<String, String>,
    /// The project's code sent to a thread from the Explorer, waiting with its message.
    pub references: HashMap<String, Vec<Reference>>,
    /// Open this thread (its agent, its id) once its agent lists it.
    pub reopen: Option<(String, protocol::ThreadId)>,
    /// A new chat's thread is being made (an insert or a message came before it existed).
    pub creating: bool,
}

impl ThreadStore {
    pub fn new(reopen: Option<(String, protocol::ThreadId)>) -> Self {
        Self {
            list: vec![],
            cache: HashMap::new(),
            drafts: super::drafts::load_drafts(),
            references: HashMap::new(),
            reopen,
            creating: false,
        }
    }
}
