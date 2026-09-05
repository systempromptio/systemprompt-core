//! The runtime guard that keeps `reasoning_tokens` a breakdown of
//! `output_tokens` for providers that were never probed.

use systemprompt_models::wire::canonical::{CanonicalUsage, CanonicalUsageUpdate};

fn usage(input: u32, output: u32, reasoning: u32, total: u32) -> CanonicalUsage {
    CanonicalUsage {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        reasoning_tokens: reasoning,
        total_tokens: total,
    }
}

#[test]
fn a_conforming_openai_payload_is_left_untouched() {
    let mut u = usage(20, 106, 100, 126);
    assert!(!u.normalise_reasoning("openai"));
    assert_eq!(u.output_tokens, 106);
    assert_eq!(u.total_tokens, 126);
}

#[test]
fn reasoning_exceeding_output_is_folded_in() {
    let mut u = usage(20, 40, 100, 0);
    assert!(u.normalise_reasoning("cerebras"));
    assert_eq!(u.output_tokens, 140);
    assert_eq!(u.reasoning_tokens, 100);
    assert_eq!(u.total_tokens, 160);
}

#[test]
fn a_wire_total_that_counts_reasoning_separately_is_folded_in() {
    let mut u = usage(27, 6, 194, 227);
    assert!(u.normalise_reasoning("moonshot"));
    assert_eq!(u.output_tokens, 200);
    assert_eq!(u.total_tokens, 227);
}

#[test]
fn gemini_thoughts_already_summed_into_output_are_not_folded_twice() {
    let mut u = usage(100, 250, 150, 350);
    assert!(!u.normalise_reasoning("gemini"));
    assert_eq!(u.output_tokens, 250);
}

#[test]
fn an_anthropic_total_including_cache_is_not_mistaken_for_additive_reasoning() {
    let mut u = CanonicalUsage {
        input_tokens: 10,
        output_tokens: 20,
        cache_read_tokens: 30,
        cache_creation_tokens: 0,
        reasoning_tokens: 0,
        total_tokens: 60,
    };
    assert!(!u.normalise_reasoning("anthropic"));
    assert_eq!(u.output_tokens, 20);
}

#[test]
fn the_streaming_fold_leaves_a_conforming_stream_alone() {
    let mut u = CanonicalUsage::default();
    CanonicalUsageUpdate {
        input_tokens: Some(20),
        output_tokens: Some(106),
        cache_read_tokens: None,
        cache_creation_tokens: None,
        reasoning_tokens: Some(100),
    }
    .apply_to(&mut u);
    assert!(!u.normalise_reasoning("openai"));
    assert_eq!(u.output_tokens, 106);
}

#[test]
fn the_streaming_fold_still_catches_reasoning_exceeding_output() {
    let mut u = CanonicalUsage::default();
    CanonicalUsageUpdate {
        input_tokens: Some(20),
        output_tokens: Some(40),
        cache_read_tokens: None,
        cache_creation_tokens: None,
        reasoning_tokens: Some(100),
    }
    .apply_to(&mut u);
    assert!(u.normalise_reasoning("cerebras"));
    assert_eq!(u.output_tokens, 140);
}

#[test]
fn cache_totals_recomputed_by_the_stream_do_not_trigger_a_fold() {
    let mut u = CanonicalUsage::default();
    CanonicalUsageUpdate {
        input_tokens: Some(10),
        output_tokens: Some(200),
        cache_read_tokens: Some(64),
        cache_creation_tokens: None,
        reasoning_tokens: Some(64),
    }
    .apply_to(&mut u);
    assert!(!u.normalise_reasoning("anthropic"));
    assert_eq!(u.output_tokens, 200);
}
