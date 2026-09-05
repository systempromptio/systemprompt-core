//! The one cost function.
//!
//! Every billable path — the gateway and the internal agent path alike —
//! prices a request by calling [`ModelPricing::cost_microdollars`] with a
//! [`CanonicalUsage`]. Taking the usage type directly is what makes a second
//! arithmetic impossible: the four billable counts arrive already disjoint
//! (`input_tokens` excludes cache reads, `reasoning_tokens` is inside
//! `output_tokens`), so no caller can transpose two same-typed arguments or
//! quietly price a subset of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::model::ModelPricing;
use crate::wire::canonical::CanonicalUsage;

impl ModelPricing {
    #[must_use]
    pub fn cost_microdollars(&self, usage: &CanonicalUsage) -> i64 {
        let rate = |count: u32, per_million: f64| (f64::from(count) / 1_000_000.0) * per_million;
        let total = rate(usage.input_tokens, self.input_per_million)
            + rate(usage.output_tokens, self.output_per_million)
            + rate(usage.cache_read_tokens, self.cache_read_per_million)
            + rate(usage.cache_creation_tokens, self.cache_write_per_million);
        (total * 1_000_000.0).round() as i64
    }
}
