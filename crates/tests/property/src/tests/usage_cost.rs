use proptest::prelude::*;
use systemprompt_models::services::ModelPricing;
use systemprompt_models::wire::canonical::CanonicalUsage;

prop_compose! {
    fn arb_usage()(
        input in 0u32..1_000_000,
        output in 0u32..1_000_000,
        cache_read in 0u32..1_000_000,
        cache_creation in 0u32..1_000_000,
        reasoning in 0u32..1_000_000,
        total in 0u32..4_000_000,
    ) -> CanonicalUsage {
        CanonicalUsage {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            cache_creation_tokens: cache_creation,
            reasoning_tokens: reasoning,
            total_tokens: total,
        }
    }
}

prop_compose! {
    fn arb_pricing()(
        input in 0.0f64..1_000.0,
        output in 0.0f64..1_000.0,
        cache_read in 0.0f64..1_000.0,
        cache_write in 0.0f64..1_000.0,
    ) -> ModelPricing {
        ModelPricing {
            input_per_million: input,
            output_per_million: output,
            cache_read_per_million: cache_read,
            cache_write_per_million: cache_write,
            per_image_cents: None,
        }
    }
}

proptest! {
    // tokens_used counts every billable class, so it can never undercount the
    // two that every provider reports.
    #[test]
    fn billable_total_is_at_least_input_plus_output(usage in arb_usage()) {
        prop_assert!(usage.billable_total() >= usage.input_tokens.saturating_add(usage.output_tokens));
    }

    // Reasoning is inside output_tokens, so it never enlarges the total.
    #[test]
    fn billable_total_ignores_reasoning(usage in arb_usage()) {
        let mut without = usage;
        without.reasoning_tokens = 0;
        prop_assert_eq!(usage.billable_total(), without.billable_total());
    }

    // The guard is a normaliser: applying it to already-normalised usage must
    // not move the counts again, or a re-audited row drifts every pass.
    #[test]
    fn normalise_reasoning_is_idempotent(usage in arb_usage()) {
        let mut once = usage;
        once.normalise_reasoning("cerebras");
        let mut twice = once;
        twice.normalise_reasoning("cerebras");
        prop_assert_eq!(once.input_tokens, twice.input_tokens);
        prop_assert_eq!(once.output_tokens, twice.output_tokens);
        prop_assert_eq!(once.reasoning_tokens, twice.reasoning_tokens);
        prop_assert_eq!(once.total_tokens, twice.total_tokens);
    }

    // Cost is monotone in every billable count: raising one count at a
    // non-negative rate can never lower the bill.
    #[test]
    fn cost_is_monotone_in_every_count(
        usage in arb_usage(),
        pricing in arb_pricing(),
        bump in 1u32..10_000,
    ) {
        let base = pricing.cost_microdollars(&usage);
        for raised in [
            CanonicalUsage { input_tokens: usage.input_tokens.saturating_add(bump), ..usage },
            CanonicalUsage { output_tokens: usage.output_tokens.saturating_add(bump), ..usage },
            CanonicalUsage { cache_read_tokens: usage.cache_read_tokens.saturating_add(bump), ..usage },
            CanonicalUsage {
                cache_creation_tokens: usage.cache_creation_tokens.saturating_add(bump),
                ..usage
            },
        ] {
            prop_assert!(pricing.cost_microdollars(&raised) >= base);
        }
    }

    // Reasoning is priced through output_tokens alone, so changing it must
    // never change the bill.
    #[test]
    fn cost_is_independent_of_reasoning(usage in arb_usage(), pricing in arb_pricing(), r in 0u32..1_000_000) {
        let with_reasoning = CanonicalUsage { reasoning_tokens: r, ..usage };
        prop_assert_eq!(
            pricing.cost_microdollars(&with_reasoning),
            pricing.cost_microdollars(&CanonicalUsage { reasoning_tokens: 0, ..usage })
        );
    }
}
