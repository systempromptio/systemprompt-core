//! [`PlaneSnapshot`]: a plane's entities, keyed by entity key, each held as
//! its canonical value text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::collections::btree_map::{IntoIter, Iter, Keys};

/// One side of a drift comparison: every entity a declaration or the database
/// holds, keyed by its entity key, valued by its canonical encoding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaneSnapshot(BTreeMap<String, String>);

impl PlaneSnapshot {
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn insert(&mut self, entity: String, value: String) -> Option<String> {
        self.0.insert(entity, value)
    }

    pub fn remove(&mut self, entity: &str) -> Option<String> {
        self.0.remove(entity)
    }

    #[must_use]
    pub fn get(&self, entity: &str) -> Option<&String> {
        self.0.get(entity)
    }

    #[must_use]
    pub fn contains_key(&self, entity: &str) -> bool {
        self.0.contains_key(entity)
    }

    pub fn keys(&self) -> Keys<'_, String, String> {
        self.0.keys()
    }

    pub fn iter(&self) -> Iter<'_, String, String> {
        self.0.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<(String, String)> for PlaneSnapshot {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl Extend<(String, String)> for PlaneSnapshot {
    fn extend<I: IntoIterator<Item = (String, String)>>(&mut self, iter: I) {
        self.0.extend(iter);
    }
}

impl IntoIterator for PlaneSnapshot {
    type Item = (String, String);
    type IntoIter = IntoIter<String, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a PlaneSnapshot {
    type Item = (&'a String, &'a String);
    type IntoIter = Iter<'a, String, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
