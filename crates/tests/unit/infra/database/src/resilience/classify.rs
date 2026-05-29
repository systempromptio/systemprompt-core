//! Tests for `Outcome` and error classification.

use std::time::Duration;

use systemprompt_database::resilience::classify::Outcome;

#[test]
fn success_is_not_transient() {
    assert!(!Outcome::Success.is_transient());
}

#[test]
fn permanent_is_not_transient() {
    assert!(!Outcome::Permanent.is_transient());
}

#[test]
fn transient_without_hint_is_transient() {
    assert!(Outcome::Transient { retry_after: None }.is_transient());
}

#[test]
fn transient_with_hint_is_transient() {
    let outcome = Outcome::Transient {
        retry_after: Some(Duration::from_millis(500)),
    };
    assert!(outcome.is_transient());
}

#[test]
fn outcome_equality_success() {
    assert_eq!(Outcome::Success, Outcome::Success);
}

#[test]
fn outcome_equality_permanent() {
    assert_eq!(Outcome::Permanent, Outcome::Permanent);
}

#[test]
fn outcome_equality_transient_none() {
    assert_eq!(
        Outcome::Transient { retry_after: None },
        Outcome::Transient { retry_after: None }
    );
}

#[test]
fn outcome_inequality_success_vs_permanent() {
    assert_ne!(Outcome::Success, Outcome::Permanent);
}

#[test]
fn outcome_debug_format() {
    let debug = format!("{:?}", Outcome::Transient { retry_after: None });
    assert!(debug.contains("Transient"));
}

#[test]
fn outcome_debug_success() {
    let debug = format!("{:?}", Outcome::Success);
    assert!(debug.contains("Success"));
}

#[test]
fn outcome_copy() {
    let original = Outcome::Permanent;
    let copy = original;
    assert_eq!(original, copy);
}
