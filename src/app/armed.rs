//! What waits for a second press before it is done: deleting a thread, a project, a file, a task,
//! an agent or an endpoint; removing a part of a project; closing a tab with unsaved changes;
//! restarting the agent over running work (`restart`). One
//! thing at a time in the window: arming another, or Esc, lets the first one go. Keys name the
//! kind of thing first (`thread:<id>`, `p:<id>`, `del:<file>`, `close:<tab>`, `env:<project>:…`).

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Armed(Option<String>);

impl Armed {
    /// Whether `key` waits for its second press (its button says so).
    pub fn is(&self, key: &str) -> bool {
        self.0.as_deref() == Some(key)
    }

    /// The first press: `key` waits for the second.
    pub fn arm(&mut self, key: impl Into<String>) {
        self.0 = Some(key.into());
    }

    /// A press of `key`: true when it was armed (the action goes now), else it is armed by it.
    pub fn confirm(&mut self, key: impl Into<String>) -> bool {
        let key = key.into();
        if self.is(&key) {
            self.0 = None;
            true
        } else {
            self.0 = Some(key);
            false
        }
    }

    /// Nothing waits any more; true when something did.
    pub fn clear(&mut self) -> bool {
        self.0.take().is_some()
    }
}

/// The key of a thread waiting to be deleted.
pub fn thread_key(id: &str) -> String {
    format!("thread:{id}")
}

#[cfg(test)]
mod tests {
    use super::Armed;

    #[test]
    fn the_second_press_of_the_same_key_goes() {
        let mut a = Armed::default();
        assert!(!a.confirm("p:1"));
        assert!(a.is("p:1"));
        assert!(!a.confirm("p:2"), "another key arms instead");
        assert!(a.confirm("p:2"));
        assert!(!a.is("p:2"));
        assert!(!a.clear());
    }
}
