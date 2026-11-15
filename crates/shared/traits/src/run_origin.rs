//! A run origin an API caller may queue a run under.
//!
//! A run is drained only by a claim loop keyed on its origin kind, so the API
//! refuses an origin no admitted member contributed: such a run would sit
//! queued forever. A member that owns a claim loop contributes its origin with
//! `Contributions::run_origin`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunOrigin {
    pub name: &'static str,
}

impl RunOrigin {
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }
}
