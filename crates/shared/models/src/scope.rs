//! Per-request scoping identity carried from middleware to scoped database
//! transactions.
//!
//! A [`RequestScope`] is dumb data: ordered key/value pairs an extension's
//! middleware populates (for example the requesting user's organization) and a
//! `ConnectionScopeProvider` in `systemprompt-database` later translates into
//! transaction-local Postgres settings for row-level security.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestScope {
    entries: Vec<(String, String)>,
}

impl RequestScope {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
