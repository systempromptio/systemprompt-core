//! Builders for the token-usage types.
//!
//! Every test that asserts on usage or cost used to spell out a struct
//! literal, so adding one count to `CanonicalUsage` broke unrelated files in
//! three crates at once. These builders take the fields a test actually cares
//! about and default the rest, which is what makes a new count a one-line
//! change to the type rather than a sweep.
//!
//! The counts follow the wire convention `CanonicalUsage` documents:
//! `input_tokens` excludes cache reads, and `reasoning_tokens` is a breakdown
//! of `output_tokens` rather than an addition to it.

use systemprompt_models::wire::canonical::{CanonicalUsage, CanonicalUsageUpdate};

#[derive(Debug, Default, Clone, Copy)]
pub struct UsageBuilder {
    input: u32,
    output: u32,
    cache_read: u32,
    cache_creation: u32,
    reasoning: u32,
    total: Option<u32>,
}

impl UsageBuilder {
    #[must_use]
    pub const fn input(mut self, n: u32) -> Self {
        self.input = n;
        self
    }

    #[must_use]
    pub const fn output(mut self, n: u32) -> Self {
        self.output = n;
        self
    }

    #[must_use]
    pub const fn cache_read(mut self, n: u32) -> Self {
        self.cache_read = n;
        self
    }

    #[must_use]
    pub const fn cache_creation(mut self, n: u32) -> Self {
        self.cache_creation = n;
        self
    }

    #[must_use]
    pub const fn reasoning(mut self, n: u32) -> Self {
        self.reasoning = n;
        self
    }

    // Why: an explicit total is the wire's own figure. Left unset, the build
    // computes the cache-inclusive sum, which is what an adapter does when the
    // provider reports no total.
    #[must_use]
    pub const fn total(mut self, n: u32) -> Self {
        self.total = Some(n);
        self
    }

    #[must_use]
    pub fn build(self) -> CanonicalUsage {
        let total = self.total.unwrap_or_else(|| {
            self.input
                .saturating_add(self.output)
                .saturating_add(self.cache_read)
                .saturating_add(self.cache_creation)
        });
        CanonicalUsage {
            input_tokens: self.input,
            output_tokens: self.output,
            cache_read_tokens: self.cache_read,
            cache_creation_tokens: self.cache_creation,
            reasoning_tokens: self.reasoning,
            total_tokens: total,
        }
    }

    #[must_use]
    pub fn build_update(self) -> CanonicalUsageUpdate {
        let full = self.build();
        CanonicalUsageUpdate {
            input_tokens: Some(full.input_tokens),
            output_tokens: Some(full.output_tokens),
            cache_read_tokens: Some(full.cache_read_tokens),
            cache_creation_tokens: Some(full.cache_creation_tokens),
            reasoning_tokens: Some(full.reasoning_tokens),
            total_tokens: self.total,
        }
    }
}

#[must_use]
pub const fn usage() -> UsageBuilder {
    UsageBuilder {
        input: 0,
        output: 0,
        cache_read: 0,
        cache_creation: 0,
        reasoning: 0,
        total: None,
    }
}

// Why: the same builder, read as an update so a streaming test reads like the
// frame it stands for. Only fields the caller set are reported; the rest are
// zero rather than absent, matching a frame that states its whole usage.
#[must_use]
pub const fn usage_update() -> UsageBuilder {
    usage()
}
