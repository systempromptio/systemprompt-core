use systemprompt_gateway::quota::{QuotaEstimate, estimate};
use systemprompt_manifest::services::ModelPricing;
use systemprompt_wire::ModelLimits;

fn pricing() -> ModelPricing {
    ModelPricing {
        input_per_million: 3.0,
        output_per_million: 15.0,
        ..ModelPricing::default()
    }
}

fn limits(max_output_tokens: u32) -> ModelLimits {
    ModelLimits {
        max_output_tokens,
        ..ModelLimits::default()
    }
}

#[test]
fn input_is_the_body_length_over_four_rounded_up() {
    assert_eq!(estimate(0, 0, None, &pricing()).input_tokens, 0);
    assert_eq!(estimate(1, 0, None, &pricing()).input_tokens, 1);
    assert_eq!(estimate(4, 0, None, &pricing()).input_tokens, 1);
    assert_eq!(estimate(5, 0, None, &pricing()).input_tokens, 2);
    assert_eq!(estimate(4_000, 0, None, &pricing()).input_tokens, 1_000);
}

#[test]
fn output_is_max_tokens_clamped_to_the_model_ceiling() {
    assert_eq!(estimate(0, 8_192, None, &pricing()).output_tokens, 8_192);
    assert_eq!(
        estimate(0, 8_192, Some(&limits(4_096)), &pricing()).output_tokens,
        4_096
    );
    assert_eq!(
        estimate(0, 1_024, Some(&limits(4_096)), &pricing()).output_tokens,
        1_024
    );
    assert_eq!(
        estimate(0, 8_192, Some(&limits(0)), &pricing()).output_tokens,
        8_192,
        "an unknown ceiling does not clamp"
    );
}

#[test]
fn cost_prices_both_estimates_with_the_pinned_rate_card() {
    let got = estimate(4_000_000, 1_000_000, None, &pricing());
    assert_eq!(
        got,
        QuotaEstimate {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cost_microdollars: 18_000_000,
        }
    );
    assert_eq!(
        estimate(4_000, 500, None, &ModelPricing::default()).cost_microdollars,
        0,
        "an unpriced model reserves tokens but no cost"
    );
}
