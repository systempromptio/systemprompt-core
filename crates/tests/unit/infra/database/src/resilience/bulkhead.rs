//! Tests for `Bulkhead`.

use systemprompt_database::resilience::bulkhead::Bulkhead;

#[test]
fn admits_up_to_the_limit() {
    let bulkhead = Bulkhead::new("dep", 2);
    let _first = bulkhead.try_acquire().expect("first permit within limit");
    let _second = bulkhead.try_acquire().expect("second permit within limit");

    assert_eq!(bulkhead.limit(), 2);
    assert!(
        bulkhead.try_acquire().is_err(),
        "third acquire exceeds the limit"
    );
}

#[test]
fn rejects_once_saturated_and_recovers_when_a_permit_drops() {
    let bulkhead = Bulkhead::new("dep", 2);
    let first = bulkhead.try_acquire().expect("first permit");
    let _second = bulkhead.try_acquire().expect("second permit");

    assert!(bulkhead.try_acquire().is_err());

    drop(first);
    assert!(bulkhead.try_acquire().is_ok());
}
