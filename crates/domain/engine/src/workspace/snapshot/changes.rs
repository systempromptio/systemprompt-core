//! Pairing two sides of a comparison by case-folded key and keeping only the
//! keys that differ.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::layout::FileEntry;
use super::tree::Hashed;
use crate::workspace::paths::fold;

pub(crate) type Side<'a> = (&'a str, &'a FileEntry);

#[derive(Clone, Copy)]
pub(crate) struct Change<'a> {
    pub(crate) old: Option<Side<'a>>,
    pub(crate) new: Option<Side<'a>>,
}

impl Change<'_> {
    pub(crate) fn key(&self) -> &str {
        self.new.or(self.old).map_or("", |(key, _)| key)
    }

    fn differs(&self) -> bool {
        match (self.old, self.new) {
            (Some((_, a)), Some((_, b))) => a.hash != b.hash || a.executable != b.executable,
            _ => true,
        }
    }
}

pub(crate) fn entries(files: &BTreeMap<String, FileEntry>) -> impl Iterator<Item = Side<'_>> {
    files.iter().map(|(key, entry)| (key.as_str(), entry))
}

pub(crate) fn working_entries(files: &BTreeMap<String, Hashed>) -> impl Iterator<Item = Side<'_>> {
    files
        .iter()
        .map(|(key, hashed)| (key.as_str(), &hashed.entry))
}

pub(crate) fn pair<'a>(
    old: impl Iterator<Item = Side<'a>>,
    new: impl Iterator<Item = Side<'a>>,
) -> Vec<Change<'a>> {
    let mut by_key: BTreeMap<String, Change<'a>> = BTreeMap::new();
    for side in old {
        by_key
            .entry(fold(side.0))
            .or_insert(Change {
                old: None,
                new: None,
            })
            .old = Some(side);
    }
    for side in new {
        by_key
            .entry(fold(side.0))
            .or_insert(Change {
                old: None,
                new: None,
            })
            .new = Some(side);
    }
    by_key.into_values().filter(Change::differs).collect()
}
