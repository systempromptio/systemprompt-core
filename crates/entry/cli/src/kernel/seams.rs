//! Link-time registration seams, as `plugins seams` lists them.
//!
//! Every seam a crate exposes through `inventory` is described by one
//! [`LinkedSeamRegistration`], submitted with
//! [`register_linked_seam!`](crate::register_linked_seam) by the crate that
//! links the seam into the CLI: the kernel for its own seams, a capability's
//! entry crate for the capability's. The registration names the seam, the
//! crate that defines it, the extension that owns it (`None` for a seam the
//! kernel owns) and a function that enumerates what this binary registers in
//! it as [`SeamEntry`] values, rendered by [`named`], [`prioritized`],
//! [`unnamed`] or, for a seam whose entries are each switched on by their own
//! registering extension, [`owned`]. An entry's own owner applies on top of
//! the seam's: an entry is active only while both are enabled. The listing
//! reads every registration through [`registered_seams`], ordered by name, so
//! no central list names a capability.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// One `inventory` submission: a registration seam and how to enumerate it.
#[derive(Debug, Clone, Copy)]
pub struct LinkedSeamRegistration {
    pub name: &'static str,
    pub owner: &'static str,
    pub extension: Option<&'static str>,
    pub entries: fn() -> Vec<SeamEntry>,
}

/// One registration in a seam, with the extension that switches it on when
/// that is decided per entry rather than by the seam's owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamEntry {
    pub name: String,
    pub owner: Option<&'static str>,
}

impl SeamEntry {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            owner: None,
        }
    }

    #[must_use]
    pub fn owned_by(name: impl Into<String>, owner: &'static str) -> Self {
        Self {
            name: name.into(),
            owner: Some(owner),
        }
    }
}

inventory::collect!(LinkedSeamRegistration);

#[macro_export]
macro_rules! register_linked_seam {
    ($registration:expr) => {
        $crate::__inventory::submit! { $registration }
    };
}

pub fn registered_seams() -> Vec<&'static LinkedSeamRegistration> {
    let mut seams: Vec<&'static LinkedSeamRegistration> = inventory::iter::<LinkedSeamRegistration>
        .into_iter()
        .collect();
    seams.sort_by_key(|seam| seam.name);
    seams
}

pub fn named<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Vec<SeamEntry> {
    sorted(names.into_iter().map(SeamEntry::new).collect())
}

pub fn owned<S: Into<String>>(
    entries: impl IntoIterator<Item = (S, &'static str)>,
) -> Vec<SeamEntry> {
    sorted(
        entries
            .into_iter()
            .map(|(name, owner)| SeamEntry::owned_by(name, owner))
            .collect(),
    )
}

pub fn prioritized(priorities: impl IntoIterator<Item = i32>) -> Vec<SeamEntry> {
    priorities
        .into_iter()
        .map(|priority| SeamEntry::new(format!("priority={priority}")))
        .collect()
}

pub fn unnamed(count: usize) -> Vec<SeamEntry> {
    (1..=count)
        .map(|n| SeamEntry::new(format!("unnamed#{n}")))
        .collect()
}

fn sorted(mut entries: Vec<SeamEntry>) -> Vec<SeamEntry> {
    entries.sort_by(|a, b| a.name.cmp(&b.name).then(a.owner.cmp(&b.owner)));
    entries
}
