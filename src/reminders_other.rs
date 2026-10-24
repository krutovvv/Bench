//! Tasks where Apple Reminders doesn't exist (Linux, Windows, the browser): the Tasks mode says
//! so instead of loading. Same interface as src/reminders.rs (macOS), so nothing else changes.

use anyhow::{Result, bail};
use chrono::NaiveDate;

use crate::tasks::Task;

pub enum Cmd {
    Load { done_since: NaiveDate },
    Save { task: Task, token: u64 },
    Delete(String),
}

pub enum Evt {
    Loaded(Vec<Task>),
    Created { token: u64, id: String },
    Error(String),
}

const ONLY_MAC: &str = "Tasks live in Apple Reminders, which is on macOS only";

pub struct Reminders;

impl Reminders {
    pub fn start(evt: async_channel::Sender<Evt>) -> Self {
        let _ = evt.try_send(Evt::Error(ONLY_MAC.into()));
        Reminders
    }

    pub fn send(&self, _cmd: Cmd) {}
}

pub fn probe() -> Result<String> {
    bail!(ONLY_MAC)
}
