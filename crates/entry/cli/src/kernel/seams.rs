//! Link-time registration seams, as `plugins seams` lists them.
//!
//! Every seam a crate exposes through `inventory` is described by one
//! [`LinkedSeamRegistration`], submitted with
//! [`register_linked_seam!`](crate::register_linked_seam) by the crate that
//! links the seam into the CLI: the kernel for its own seams, a capability's
//! entry crate for the capability's. The registration names the seam, the
//! crate that defines it, the extension that owns it (`None` for a seam the
//! kernel owns) and a function that enumerates what this binary registers in
//! it, rendered by [`named`], [`prioritized`] or [`unnamed`]. The listing
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
    pub entries: fn() -> Vec<String>,
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

pub fn named<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Vec<String> {
    let mut out: Vec<String> = names.into_iter().map(Into::into).collect();
    out.sort();
    out
}

pub fn prioritized(priorities: impl IntoIterator<Item = i32>) -> Vec<String> {
    priorities
        .into_iter()
        .map(|priority| format!("priority={priority}"))
        .collect()
}

pub fn unnamed(count: usize) -> Vec<String> {
    (1..=count).map(|n| format!("unnamed#{n}")).collect()
}
