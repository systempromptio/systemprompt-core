//! Unit tests for the email-validation helper exposed by the API services
//! module — covers shape rules (single `@`, dotted domain, no leading/trailing
//! dot) and whitespace trimming.

use systemprompt_api::services::validation::is_valid_email;

#[test]
fn accepts_basic_email() {
    assert!(is_valid_email("alice@example.com"));
    assert!(is_valid_email("a@b.co"));
}

#[test]
fn accepts_email_with_subdomain() {
    assert!(is_valid_email("user@mail.example.com"));
}

#[test]
fn trims_surrounding_whitespace() {
    assert!(is_valid_email("  alice@example.com  "));
    assert!(is_valid_email("\talice@example.com\n"));
}

#[test]
fn rejects_missing_at() {
    assert!(!is_valid_email("alice.example.com"));
    assert!(!is_valid_email("alice"));
    assert!(!is_valid_email(""));
}

#[test]
fn rejects_empty_local_or_domain() {
    assert!(!is_valid_email("@example.com"));
    assert!(!is_valid_email("alice@"));
    assert!(!is_valid_email("@"));
}

#[test]
fn rejects_multiple_at_signs() {
    assert!(!is_valid_email("a@b@c.com"));
    assert!(!is_valid_email("a@@example.com"));
}

#[test]
fn rejects_domain_without_dot() {
    assert!(!is_valid_email("alice@localhost"));
    assert!(!is_valid_email("alice@server"));
}

#[test]
fn rejects_leading_or_trailing_dot_in_domain() {
    assert!(!is_valid_email("alice@.example.com"));
    assert!(!is_valid_email("alice@example.com."));
}
