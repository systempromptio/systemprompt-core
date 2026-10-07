//! The token and cost estimate a request reserves at admission.
//!
//! Input is approximated from the body length (four bytes a token, rounded
//! up); output is the request's `max_tokens`, clamped to the served model's
//! output ceiling; cost prices both with the rate card pinned for the request.
//! The estimate only has to bound the request: completion trues the bucket up
//! to the audited usage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::ModelPricing;
use systemprompt_wire::ModelLimits;
use systemprompt_wire::canonical::CanonicalUsage;

const BYTES_PER_TOKEN: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuotaEstimate {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cost_microdollars: i64,
}

#[must_use]
pub fn estimate(
    raw_body_len: usize,
    max_tokens: u32,
    model_limits: Option<&ModelLimits>,
    pricing: &ModelPricing,
) -> QuotaEstimate {
    let input_tokens = u32::try_from(raw_body_len.div_ceil(BYTES_PER_TOKEN)).unwrap_or(u32::MAX);
    let output_tokens = systemprompt_wire::clamp_output_tokens(
        max_tokens,
        model_limits.map(|l| l.max_output_tokens),
    );
    let usage = CanonicalUsage {
        input_tokens,
        output_tokens,
        ..CanonicalUsage::default()
    };
    QuotaEstimate {
        input_tokens,
        output_tokens,
        cost_microdollars: pricing.cost_microdollars(&usage),
    }
}
