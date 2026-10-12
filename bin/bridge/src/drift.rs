//! The one comparison between what a writer would put in a host's settings
//! and what the host actually holds.
//!
//! Every writer renders a [`DesiredProfile`] and every probe reads the same
//! keys back; [`diff`] is the only place the two are compared, so the agent
//! row, the sync's own conflict check and `doctor` cannot judge the same
//! settings by different rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;

/// What a desired key is for, so a drift can name its cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRole {
    Secret,
    Endpoint,
    Models,
    ManagedServers,
    Setting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredEntry {
    pub key: String,
    pub value: String,
    pub role: EntryRole,
}

/// Every key a host's writer sets, with the value it sets it to.
///
/// A secret is carried as its fingerprint, never the value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesiredProfile {
    entries: Vec<DesiredEntry>,
}

impl DesiredProfile {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>, role: EntryRole) {
        self.entries.push(DesiredEntry {
            key: key.into(),
            value: value.into(),
            role,
        });
    }

    #[must_use]
    pub fn entries(&self) -> &[DesiredEntry] {
        &self.entries
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.key.as_str())
    }

    #[must_use]
    pub fn role_of(&self, key: &str) -> EntryRole {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map_or(EntryRole::Setting, |e| e.role)
    }
}

/// The keys a host is missing or holds a different value for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Drift {
    pub missing: Vec<String>,
    pub differing: Vec<String>,
}

impl Drift {
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.differing.is_empty()
    }

    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let mut keys = self.missing.clone();
        keys.extend(self.differing.iter().cloned());
        keys
    }
}

#[must_use]
pub fn diff(desired: &DesiredProfile, observed: &BTreeMap<String, String>) -> Drift {
    let mut drift = Drift::default();
    for entry in &desired.entries {
        match observed.get(&entry.key) {
            None => drift.missing.push(entry.key.clone()),
            Some(have) if !equivalent(&entry.value, have) => {
                drift.differing.push(entry.key.clone());
            },
            Some(_) => {},
        }
    }
    drift
}

#[must_use]
pub fn differing_names<V: PartialEq>(
    desired: &[(String, V)],
    observed: &BTreeMap<String, V>,
) -> Vec<String> {
    desired
        .iter()
        .filter(|(name, value)| observed.get(name) != Some(value))
        .map(|(name, _)| name.clone())
        .collect()
}

// Why: hosts hand structured values back re-serialised (a registry JSON
// string, a plist dict read as JSON, a TOML inline table), so two texts that
// encode the same JSON value are the same setting.
// JSON: host settings values — structured values compared as parsed JSON.
fn equivalent(want: &str, have: &str) -> bool {
    if want == have {
        return true;
    }
    match (
        serde_json::from_str::<serde_json::Value>(want),
        serde_json::from_str::<serde_json::Value>(have),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
