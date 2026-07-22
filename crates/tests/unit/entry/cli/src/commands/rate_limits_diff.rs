//! Tests for `admin config rate-limits diff`: field-by-field comparison of a
//! rate-limits config against another, including tier multipliers.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::rate_limits::diff::collect_differences;
use systemprompt_models::RateLimitsConfig;

#[test]
fn identical_configs_produce_no_differences() {
    let a = RateLimitsConfig::default();
    let b = RateLimitsConfig::default();
    assert!(collect_differences(&a, &b).is_empty());
}

#[test]
fn changed_scalar_fields_are_reported_with_both_values() {
    let current = RateLimitsConfig::default();
    let mut other = RateLimitsConfig::default();
    other.disabled = !current.disabled;
    other.tasks_per_second = current.tasks_per_second + 7;

    let diffs = collect_differences(&current, &other);
    assert_eq!(diffs.len(), 2);

    let disabled = diffs.iter().find(|d| d.field == "disabled").unwrap();
    assert_eq!(disabled.current, current.disabled.to_string());
    assert_eq!(disabled.other, other.disabled.to_string());

    let tasks = diffs
        .iter()
        .find(|d| d.field == "tasks_per_second")
        .unwrap();
    assert_eq!(tasks.other, other.tasks_per_second.to_string());
}

#[test]
fn tier_multiplier_changes_use_dotted_field_names_and_one_decimal() {
    let current = RateLimitsConfig::default();
    let mut other = RateLimitsConfig::default();
    other.tier_multipliers.admin = current.tier_multipliers.admin + 2.5;

    let diffs = collect_differences(&current, &other);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].field, "tier_multipliers.admin");
    assert_eq!(
        diffs[0].other,
        format!("{:.1}", other.tier_multipliers.admin)
    );
}

#[test]
fn equal_tier_multipliers_within_epsilon_are_not_reported() {
    let current = RateLimitsConfig::default();
    let mut other = RateLimitsConfig::default();
    other.tier_multipliers.user = current.tier_multipliers.user + f64::EPSILON / 2.0;
    assert!(collect_differences(&current, &other).is_empty());
}
