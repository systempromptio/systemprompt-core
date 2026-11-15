//! The run origins an API caller may queue under.
//!
//! A run is drained only by a `ClaimLoop` keyed on its `origin_kind`, so the
//! HTTP API refuses an origin no admitted member declared: such a run would
//! sit queued forever. A member that owns a claim loop declares its origin
//! with `c.run_origin(RunOrigin::new(..))`; [`AdmittedRunOrigins`] is the set
//! the admitted members declared, so a disabled member's origins are refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::BuiltState;

/// Every admitted member's declared run origins, sorted and deduplicated.
#[derive(Debug, Clone, Default)]
pub struct AdmittedRunOrigins {
    names: Vec<&'static str>,
}

impl AdmittedRunOrigins {
    #[must_use]
    pub fn of(built: &BuiltState) -> Self {
        let mut names: Vec<&'static str> = built
            .members()
            .flat_map(|member| member.sink.run_origins().iter().map(|origin| origin.name))
            .collect();
        names.sort_unstable();
        names.dedup();
        Self { names }
    }

    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(&name)
    }

    #[must_use]
    pub fn names(&self) -> &[&'static str] {
        &self.names
    }
}
