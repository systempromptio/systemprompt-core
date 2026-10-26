//! The registry of run origins an API caller may queue under.
//!
//! A run is drained only by a `ClaimLoop` keyed on its `origin_kind`, so the
//! HTTP API refuses an origin nobody registered: such a run would sit queued
//! forever. An extension that owns a claim loop registers its origin with
//! [`register_run_origin!`](crate::register_run_origin).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// One registered origin kind, submitted through
/// [`register_run_origin!`](crate::register_run_origin).
#[derive(Debug, Clone, Copy)]
pub struct RunOriginRegistration {
    pub name: &'static str,
}

inventory::collect!(RunOriginRegistration);

#[macro_export]
macro_rules! register_run_origin {
    ($name:expr $(,)?) => {
        $crate::__inventory::submit! {
            $crate::origin::RunOriginRegistration { name: $name }
        }
    };
}

#[must_use]
pub fn is_registered(name: &str) -> bool {
    inventory::iter::<RunOriginRegistration>().any(|origin| origin.name == name)
}

#[must_use]
pub fn registered() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = inventory::iter::<RunOriginRegistration>()
        .map(|origin| origin.name)
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}
