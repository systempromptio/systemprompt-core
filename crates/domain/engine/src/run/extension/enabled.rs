//! The extensions an instance turned on for this host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use harness_ids::ExtensionId;

/// The ids an instance enabled for this host.
///
/// It is only ever read as `Option<&EnabledExtensions>`: `None` means no
/// instance said anything (not signed in, or a manifest without the host's
/// entry), and then only required extensions run. An id missing from the set
/// is withheld.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnabledExtensions(BTreeSet<ExtensionId>);

impl EnabledExtensions {
    pub fn contains(&self, id: &ExtensionId) -> bool {
        self.0.contains(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ExtensionId> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<ExtensionId> for EnabledExtensions {
    fn from_iter<I: IntoIterator<Item = ExtensionId>>(ids: I) -> Self {
        Self(ids.into_iter().collect())
    }
}
