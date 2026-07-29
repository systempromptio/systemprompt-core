//! Tests for the canonical email form applied at every create and lookup.

use systemprompt_users::normalise_email;

#[test]
fn lowercases_and_trims() {
    assert_eq!(normalise_email("  Ed@Example.COM \n"), "ed@example.com");
}

#[test]
fn is_idempotent() {
    let once = normalise_email("  MiXeD@Case.Example  ");
    assert_eq!(normalise_email(&once), once);
}

#[test]
fn leaves_an_already_canonical_address_untouched() {
    assert_eq!(normalise_email("ed@example.com"), "ed@example.com");
}

#[test]
fn preserves_the_local_part_beyond_case() {
    assert_eq!(
        normalise_email("First.Last+tag@Example.com"),
        "first.last+tag@example.com"
    );
}
