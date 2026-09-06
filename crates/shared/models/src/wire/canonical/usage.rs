//! The canonical token-usage types and their conventions.
//!
//! # Reasoning tokens
//!
//! `CanonicalUsage::reasoning_tokens` is a **breakdown of** `output_tokens`,
//! never an addition to it. Providers disagree on the wire, so every adapter
//! normalises to that one rule before the count reaches billing:
//!
//! * `OpenAI` (chat and responses) already folds
//!   `*_tokens_details.reasoning_tokens` into `completion_tokens` /
//!   `output_tokens`, so the adapter copies it across untouched.
//! * Gemini reports `thoughtsTokenCount` *beside* `candidatesTokenCount` (and
//!   inside `totalTokenCount`), so its adapter adds it into `output_tokens` on
//!   the way in.
//! * Anthropic bills thinking as ordinary output tokens and, for adaptive
//!   thinking on Claude 5 models, reports the share as
//!   `usage.output_tokens_details.thinking_tokens`; the adapter copies it
//!   across untouched. Models that report no details yield 0.
//!
//! Holding that invariant here is what makes reasoning billable: cost is
//! computed from `output_tokens`, so a reasoning-only turn is charged at the
//! output rate with no per-provider arithmetic downstream, and no count is
//! charged twice. It is also why `reasoning_tokens` is absent from the
//! `total_tokens` sum in [`CanonicalUsageUpdate::apply_to`].
//!
//! Third-party `OpenAI`-compatible upstreams (Cerebras, Moonshot, Qwen) are not
//! probed, so `CanonicalUsage::normalise_reasoning` enforces the rule at
//! runtime rather than trusting it: a breakdown cannot exceed its parent, and a
//! wire `total_tokens` that overshoots `input + output` by exactly the
//! reasoning count is the same signal: because `input_tokens` excludes cache
//! reads, an additive provider's wire total is exactly `billable_total() +
//! reasoning_tokens`, while a conforming one states `billable_total()` alone.
//! Either signal means the provider reported reasoning *additionally*, so the
//! count is folded into `output_tokens` and warned about. The total-based half
//! fires on both paths: [`CanonicalUsageUpdate`] carries the wire's own
//! `total_tokens` when a frame states one, and
//! [`CanonicalUsageUpdate::apply_to`] recomputes only when it does not — and a
//! recomputed total is `billable_total()`, which is never the additive shape.
//!
//! # Cache tokens
//!
//! `input_tokens` is **exclusive** of `cache_read_tokens` on every wire.
//! Anthropic reports the two disjointly; `OpenAI`, Gemini and the
//! `OpenAI`-compatible upstreams report the cached count as a *subset* of the
//! prompt count, so their adapters subtract it before it reaches this type.
//! Billing therefore charges each token exactly once, at exactly one rate, and
//! `CanonicalUsage::billable_total` is the only definition of `tokens_used`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, Default)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a token count; the `_tokens` suffix is the domain vocabulary shared \
              with the provider usage wire formats"
)]
pub struct CanonicalUsage {
    // Why: exclusive of cache_read_tokens on every wire -- see the module head.
    pub input_tokens: u32,

    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,

    // Why: a breakdown of output_tokens, not an addition -- see the module
    // head for the per-provider normalisation and why billing depends on it.
    pub reasoning_tokens: u32,

    // Why: the wire's own figure when the provider states one; otherwise the
    // cache-inclusive sum. `normalise_reasoning` reads it as a signal, so it
    // must not be recomputed when the wire reported it.
    pub total_tokens: u32,
}

impl CanonicalUsage {
    // Why: the single definition of `tokens_used`. Every count is disjoint --
    // input excludes cache reads, reasoning is inside output -- so this sum
    // charges each token once and matches what cost_microdollars prices.
    #[must_use]
    pub const fn billable_total(&self) -> u32 {
        self.input_tokens
            .saturating_add(self.output_tokens)
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_creation_tokens)
    }

    // Why: enforces the module head's one rule for providers we have never
    // probed. Returns whether the count had to be folded in, so callers can
    // assert on it; the warning is emitted here so no call site can forget it.
    pub fn normalise_reasoning(&mut self, provider: &str) -> bool {
        // Why: `input_tokens` is exclusive of cache reads, so a provider that
        // counts reasoning on top of its completion states a wire total of
        // exactly `billable_total() + reasoning_tokens`. A conforming provider
        // states `billable_total()` alone, and so does a total the streaming
        // accumulator recomputed, so both fall outside this shape without
        // needing a separate exclusion.
        let additive = self.reasoning_tokens > self.output_tokens
            || (self.reasoning_tokens > 0
                && self.total_tokens
                    == self.billable_total().saturating_add(self.reasoning_tokens));
        if !additive {
            return false;
        }
        let folded = self.output_tokens.saturating_add(self.reasoning_tokens);
        tracing::warn!(
            provider,
            reasoning_tokens = self.reasoning_tokens,
            reported_output_tokens = self.output_tokens,
            folded_output_tokens = folded,
            "provider reports reasoning tokens in addition to output tokens; folding them in so \
             the thinking share is billed"
        );
        self.output_tokens = folded;
        self.total_tokens = self.billable_total();
        true
    }
}

/// A streaming usage report, carrying only the counts its frame actually
/// stated.
///
/// [`CanonicalUsage`] cannot express this: an unreported count and a reported
/// zero are both `0`. Providers differ in what a mid-stream usage frame
/// includes — an Anthropic `message_delta` may carry `output_tokens` alone —
/// so folding one in as though it were complete zeroes the input and cache
/// counts an earlier frame established, and billing loses them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a token count; the `_tokens` suffix is the domain vocabulary shared \
              with the provider usage wire formats"
)]
pub struct CanonicalUsageUpdate {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
    pub cache_creation_tokens: Option<u32>,
    pub reasoning_tokens: Option<u32>,

    // Why: the wire's own total when the frame stated one. Without it every
    // stream is billed against a recomputed sum, and `normalise_reasoning`
    // loses its total-based signal on the streaming path entirely.
    pub total_tokens: Option<u32>,
}

impl CanonicalUsageUpdate {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cache_read_tokens.is_none()
            && self.cache_creation_tokens.is_none()
            && self.reasoning_tokens.is_none()
            && self.total_tokens.is_none()
    }

    pub const fn apply_to(&self, usage: &mut CanonicalUsage) {
        if let Some(v) = self.input_tokens {
            usage.input_tokens = v;
        }
        if let Some(v) = self.output_tokens {
            usage.output_tokens = v;
        }
        if let Some(v) = self.cache_read_tokens {
            usage.cache_read_tokens = v;
        }
        if let Some(v) = self.cache_creation_tokens {
            usage.cache_creation_tokens = v;
        }
        if let Some(v) = self.reasoning_tokens {
            usage.reasoning_tokens = v;
        }
        // Why: reasoning_tokens is a subset of output_tokens, so it is
        // deliberately absent from the fallback sum -- adding it would
        // double-count every thinking turn in `total_tokens` and in the cost
        // derived from it.
        usage.total_tokens = match self.total_tokens {
            Some(v) => v,
            None => usage.billable_total(),
        };
    }
}
