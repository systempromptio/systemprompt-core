//! What an upstream has refused, remembered per provider for the rest of the
//! process: the store behind both the refused-beta and the refused-field
//! learners, so each keeps only its parser.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeSet, HashMap};
use std::sync::{OnceLock, RwLock};

type Sets = RwLock<HashMap<String, BTreeSet<String>>>;

#[derive(Debug, Default)]
pub(super) struct Learned(OnceLock<Sets>);

impl Learned {
    #[must_use]
    pub(super) const fn new() -> Self {
        Self(OnceLock::new())
    }

    fn sets(&self) -> &Sets {
        self.0.get_or_init(|| RwLock::new(HashMap::new()))
    }

    #[must_use]
    pub(super) fn for_provider(&self, provider: &str) -> BTreeSet<String> {
        self.sets()
            .read()
            .map(|sets| sets.get(provider).cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    pub(super) fn learn(&self, provider: &str, refused: &BTreeSet<String>) {
        if refused.is_empty() {
            return;
        }
        if let Ok(mut sets) = self.sets().write() {
            sets.entry(provider.to_owned())
                .or_default()
                .extend(refused.iter().cloned());
        }
    }
}
