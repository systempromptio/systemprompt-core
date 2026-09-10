//! Cost bounding for evaluation requests admitted through the gateway.
//!
//! The bound is what the budget reservation is taken against, so it must be
//! an upper bound: it prices every input byte at the most expensive of the
//! fresh, cache-read and cache-write rates, adds a fixed prompt allowance, and
//! refuses to produce a bound at all when pricing is absent or nonsensical.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_api::services::gateway::evaluation::request_bound;
use systemprompt_models::services::ModelPricing;

fn pricing(input: f64, output: f64) -> ModelPricing {
    ModelPricing {
        input_per_million: input,
        output_per_million: output,
        cache_read_per_million: None,
        cache_write_per_million: None,
        per_image_cents: None,
    }
}

#[test]
fn the_bound_grows_with_input_size_and_output_allowance() {
    let pricing = pricing(3.0, 15.0);
    let small = request_bound(&pricing, 1_000, 1_000).expect("priced request must bound");
    let more_input = request_bound(&pricing, 100_000, 1_000).expect("priced request must bound");
    let more_output = request_bound(&pricing, 1_000, 30_000).expect("priced request must bound");

    assert!(
        more_input > small,
        "a larger encoded body must raise the reservation: {more_input} vs {small}"
    );
    assert!(
        more_output > small,
        "a larger output allowance must raise the reservation: {more_output} vs {small}"
    );
}

#[test]
fn the_bound_prices_input_at_the_most_expensive_rate() {
    let mut expensive_cache = pricing(3.0, 15.0);
    expensive_cache.cache_write_per_million = Some(30.0);

    let plain = request_bound(&pricing(3.0, 15.0), 10_000, 100).expect("bound");
    let cached = request_bound(&expensive_cache, 10_000, 100).expect("bound");

    assert!(
        cached > plain,
        "a cache-write rate above the fresh rate must dominate the bound: {cached} vs {plain}"
    );
}

#[test]
fn a_zero_length_request_still_reserves_the_prompt_allowance() {
    let bound = request_bound(&pricing(3.0, 15.0), 0, 0).expect("bound");

    assert!(
        bound >= 1,
        "an admitted request always reserves at least one microdollar, got {bound}"
    );
}

#[test]
fn unpriced_models_cannot_be_admitted() {
    assert!(
        request_bound(&pricing(0.0, 0.0), 1_000, 100).is_err(),
        "a model with no pricing must not be admitted onto a budget"
    );
}

#[test]
fn nonsensical_pricing_is_refused() {
    assert!(
        request_bound(&pricing(-1.0, 15.0), 1_000, 100).is_err(),
        "a negative input rate must be refused"
    );
    assert!(
        request_bound(&pricing(3.0, f64::NAN), 1_000, 100).is_err(),
        "a non-finite output rate must be refused"
    );
    assert!(
        request_bound(&pricing(3.0, f64::INFINITY), 1_000, 100).is_err(),
        "an infinite output rate must be refused"
    );

    let mut bad_cache = pricing(3.0, 15.0);
    bad_cache.cache_read_per_million = Some(f64::NAN);
    assert!(
        request_bound(&bad_cache, 1_000, 100).is_err(),
        "a non-finite cache-read rate must be refused"
    );
}

#[test]
fn an_oversized_body_is_refused_rather_than_truncated() {
    assert!(
        request_bound(&pricing(3.0, 15.0), usize::MAX, 100).is_err(),
        "a body that cannot be measured must fail the bound, never wrap around"
    );
}

#[test]
fn the_bound_is_an_upper_bound_on_the_priced_cost() {
    let pricing = pricing(3.0, 15.0);
    let bytes = 10_000usize;
    let output = 1_000u32;
    let bound = request_bound(&pricing, bytes, output).expect("bound");
    let floor = (bytes as f64).mul_add(
        pricing.input_per_million,
        f64::from(output) * pricing.output_per_million,
    );

    assert!(
        bound as f64 >= floor,
        "the reservation must never sit below the priced cost: {bound} vs {floor}"
    );
}
