//! Unit tests for SafeIdentifier validation

use systemprompt_database::{IdentifierError, SafeIdentifier};

#[test]
fn accepts_plain_identifier() {
    let id = SafeIdentifier::parse("users").expect("plain identifier must parse");
    assert_eq!(id.as_str(), "users");
}

#[test]
fn accepts_underscore_lead() {
    assert!(SafeIdentifier::parse("_private").is_ok());
}

#[test]
fn accepts_mixed_case_and_digits() {
    assert!(SafeIdentifier::parse("Table1_Foo").is_ok());
}

#[test]
fn rejects_empty() {
    assert!(matches!(
        SafeIdentifier::parse(""),
        Err(IdentifierError::Empty)
    ));
}

#[test]
fn rejects_leading_digit() {
    assert!(matches!(
        SafeIdentifier::parse("1table"),
        Err(IdentifierError::BadLead)
    ));
}

#[test]
fn rejects_hyphen() {
    assert!(matches!(
        SafeIdentifier::parse("user-table"),
        Err(IdentifierError::InvalidChar('-'))
    ));
}

#[test]
fn rejects_whitespace() {
    assert!(matches!(
        SafeIdentifier::parse("my table"),
        Err(IdentifierError::InvalidChar(' '))
    ));
}

#[test]
fn rejects_sql_injection_attempt() {
    assert!(SafeIdentifier::parse("users\"); DROP TABLE users;--").is_err());
}

#[test]
fn rejects_too_long() {
    let raw = "a".repeat(64);
    assert!(matches!(
        SafeIdentifier::parse(&raw),
        Err(IdentifierError::TooLong(64))
    ));
}

#[test]
fn accepts_63_char_limit() {
    let raw = "a".repeat(63);
    assert!(SafeIdentifier::parse(&raw).is_ok());
}

#[test]
fn display_roundtrips() {
    let id = SafeIdentifier::parse("agent_tasks").expect("must parse");
    assert_eq!(format!("{id}"), "agent_tasks");
}
