//! The open thread's state: which session it is, its journal and the list showing it, its run,
//! what waits in its queue and composer's extras, what the agent asks in it, its project. One
//! struct, so a second thread beside the first (`Panel::Thread`) is a second `ActiveThread`.

use std::collections::HashSet;

use crate::app::Live;
use crate::app::model::Usage;

pub struct ActiveThread {
    /// Stored id of the open session (what the session list shows).
    pub session: Option<String>,
    /// The id the gateway uses for prompts and events of the open session.
    pub runtime: Option<String>,
    /// The open thread: its journal, what is being written, its queue (`crates/thread`).
    pub journal: ::thread::Thread,
    /// The open thread on screen: its list and rows (`ui::thread`).
    pub tv: crate::thread::View,
    /// Opened pieces of work and steps, by the work row's key.
    pub open_work: HashSet<String>,
    /// The open thread's texts, parsed once (`ui::media::Texts`).
    pub texts: crate::media::text::Texts,
    /// The open thread's history could not be read: why (shown with Retry instead of "Loading").
    pub history_error: Option<String>,
    pub live: Option<Live>,
    pub queued: Option<String>,
    /// The "Changed files" list under the last turn is unfolded (folded by default).
    pub files_open: bool,
    /// The open session's background processes (`thread/processes.rs`), the card's fold, and
    /// the process whose output is shown.
    pub processes: Vec<crate::app::model::BgProcess>,
    pub processes_open: bool,
    pub process_open: Option<String>,
    pub usage: Option<Usage>,
    /// The fallback model is answering the open thread: (its model, why the main one failed).
    pub fallback_on: Option<(String, String)>,
    pub effort: usize,
    /// The open session's effort when it is none of EFFORTS (set elsewhere, e.g. `high`; "" is the
    /// provider's default): shown as it is, nothing checked.
    pub effort_other: Option<String>,
    /// The agent waits for an answer (approval mode manual/smart).
    pub approval: Option<crate::app::model::ApprovalReq>,
    /// A question the agent asks, the choices ticked so far (multi-select), the free answer.
    pub clarify: Option<crate::app::model::ClarifyReq>,
    pub clarify_picked: Vec<usize>,
    /// The agent's plan for this chat, and whether its list is unfolded.
    pub todos: Vec<crate::app::model::Todo>,
    pub plan_open: bool,
    /// The open session's project (None for chats that touch no repo).
    pub project: Option<crate::app::model::Project>,
    /// The project the open thread belongs to (a new one is created in it); "" = none.
    pub thread_project: String,
}

impl Default for ActiveThread {
    fn default() -> Self {
        Self {
            session: None,
            runtime: None,
            journal: Default::default(),
            tv: Default::default(),
            open_work: Default::default(),
            texts: Default::default(),
            history_error: None,
            live: None,
            queued: None,
            files_open: false,
            processes: Vec::new(),
            processes_open: false,
            process_open: None,
            usage: None,
            fallback_on: None,
            effort: crate::app::model::EFFORTS.len() - 1,
            effort_other: None,
            approval: None,
            clarify: None,
            clarify_picked: Vec::new(),
            todos: Vec::new(),
            plan_open: false,
            project: None,
            thread_project: String::new(),
        }
    }
}
