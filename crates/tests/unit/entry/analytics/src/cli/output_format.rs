//! Tests for `analytics::shared::output` formatters — thousands grouping,
//! cost/percent/token bucketing, and percent-change computation.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::analytics::shared::{
    BreakdownData, MetricCard, format_change, format_cost, format_number, format_percent,
    format_tokens,
};

#[test]
fn format_number_groups_thousands_with_commas() {
    assert_eq!(format_number(0), "0");
    assert_eq!(format_number(42), "42");
    assert_eq!(format_number(999), "999");
    assert_eq!(format_number(1_000), "1,000");
    assert_eq!(format_number(12_345), "12,345");
    assert_eq!(format_number(1_234_567), "1,234,567");
}

#[test]
fn format_number_preserves_negative_sign() {
    assert_eq!(format_number(-1_000), "-1,000");
    assert_eq!(format_number(-12_345), "-12,345");
}

#[test]
fn format_cost_uses_four_decimals_for_sub_cent_positive() {
    assert_eq!(format_cost(5_000), "$0.0050");
    assert_eq!(format_cost(1), "$0.0000");
}

#[test]
fn format_cost_zero_falls_through_to_two_decimals() {
    assert_eq!(format_cost(0), "$0.00");
}

#[test]
fn format_cost_uses_two_decimals_under_one_hundred_dollars() {
    assert_eq!(format_cost(1_500_000), "$1.50");
    assert_eq!(format_cost(99_990_000), "$99.99");
}

#[test]
fn format_cost_drops_decimals_at_or_above_one_hundred() {
    assert_eq!(format_cost(100_000_000), "$100");
    assert_eq!(format_cost(250_500_000), "$250");
}

#[test]
fn format_percent_scales_precision_by_magnitude() {
    assert_eq!(format_percent(0.05), "0.05%");
    assert_eq!(format_percent(5.25), "5.2%");
    assert_eq!(format_percent(42.7), "43%");
}

#[test]
fn format_percent_uses_absolute_value_for_bucketing() {
    assert_eq!(format_percent(-0.05), "-0.05%");
    assert_eq!(format_percent(-42.7), "-43%");
}

#[test]
fn format_change_returns_none_when_previous_is_zero() {
    assert_eq!(format_change(10, 0), None);
}

#[test]
fn format_change_signs_positive_and_negative_deltas() {
    assert_eq!(format_change(150, 100).as_deref(), Some("+50.0%"));
    assert_eq!(format_change(100, 100).as_deref(), Some("+0.0%"));
    assert_eq!(format_change(50, 100).as_deref(), Some("-50.0%"));
}

#[test]
fn format_tokens_buckets_into_raw_k_and_m() {
    assert_eq!(format_tokens(999), "999");
    assert_eq!(format_tokens(1_500), "1.5K");
    assert_eq!(format_tokens(2_500_000), "2.5M");
}

#[test]
fn breakdown_finalize_computes_percentages_and_sorts_descending() {
    let mut data = BreakdownData::new("providers");
    data.add("openai", 30);
    data.add("anthropic", 70);
    data.finalize();

    assert_eq!(data.total, 100);
    assert_eq!(data.items[0].name, "anthropic");
    assert_eq!(data.items[0].count, 70);
    assert!((data.items[0].percentage - 70.0).abs() < f64::EPSILON);
    assert_eq!(data.items[1].name, "openai");
    assert!((data.items[1].percentage - 30.0).abs() < f64::EPSILON);
}

#[test]
fn breakdown_finalize_leaves_zero_percentages_when_total_is_zero() {
    let mut data = BreakdownData::new("empty");
    data.add("none", 0);
    data.finalize();

    assert_eq!(data.total, 0);
    assert!((data.items[0].percentage - 0.0).abs() < f64::EPSILON);
}

#[test]
fn metric_card_builders_attach_change_and_secondary() {
    let card = MetricCard::new("Requests", "1,000")
        .with_change("+10.0%")
        .with_secondary("vs last week");

    assert_eq!(card.label, "Requests");
    assert_eq!(card.value, "1,000");
    assert_eq!(card.change.as_deref(), Some("+10.0%"));
    assert_eq!(card.secondary.as_deref(), Some("vs last week"));
}

#[test]
fn metric_card_defaults_have_no_change_or_secondary() {
    let card = MetricCard::new("Total", "5");
    assert_eq!(card.change, None);
    assert_eq!(card.secondary, None);
}
