use systemprompt_identifiers::{Email, DbValue, ToDbValue};

#[test]
fn valid_simple_email() {
    let email = Email::try_new("user@example.com").unwrap();
    assert_eq!(email.as_str(), "user@example.com");
}

#[test]
fn valid_email_with_dots_in_local() {
    let email = Email::try_new("first.last@example.com").unwrap();
    assert_eq!(email.local_part(), "first.last");
}

#[test]
fn valid_email_with_plus_addressing() {
    let email = Email::try_new("user+tag@example.com").unwrap();
    assert_eq!(email.local_part(), "user+tag");
}

#[test]
fn valid_email_subdomain() {
    let email = Email::try_new("user@mail.example.co.uk").unwrap();
    assert_eq!(email.domain(), "mail.example.co.uk");
}

#[test]
fn rejects_empty_string() {
    let err = Email::try_new("").unwrap_err();
    assert_eq!(err.to_string(), "Email cannot be empty");
}

#[test]
fn rejects_no_at_symbol() {
    let err = Email::try_new("userexample.com").unwrap_err();
    assert!(err.to_string().contains("exactly one '@'"));
}

#[test]
fn rejects_multiple_at_symbols() {
    let err = Email::try_new("user@@example.com").unwrap_err();
    assert!(err.to_string().contains("exactly one '@'"));
}

#[test]
fn rejects_empty_local_part() {
    let err = Email::try_new("@example.com").unwrap_err();
    assert!(err.to_string().contains("local part"));
    assert!(err.to_string().contains("empty"));
}

#[test]
fn rejects_empty_domain_part() {
    let err = Email::try_new("user@").unwrap_err();
    assert!(err.to_string().contains("domain"));
    assert!(err.to_string().contains("empty"));
}

#[test]
fn rejects_local_starting_with_dot() {
    let err = Email::try_new(".user@example.com").unwrap_err();
    assert!(err.to_string().contains("start or end with '.'"));
}

#[test]
fn rejects_local_ending_with_dot() {
    let err = Email::try_new("user.@example.com").unwrap_err();
    assert!(err.to_string().contains("start or end with '.'"));
}

#[test]
fn rejects_consecutive_dots_in_local() {
    let err = Email::try_new("user..name@example.com").unwrap_err();
    assert!(err.to_string().contains("consecutive dots"));
}

#[test]
fn rejects_newline_in_local() {
    let err = Email::try_new("user\n@example.com").unwrap_err();
    assert!(err.to_string().contains("newline"));
}

#[test]
fn rejects_carriage_return_in_local() {
    let err = Email::try_new("user\r@example.com").unwrap_err();
    assert!(err.to_string().contains("newline"));
}

#[test]
fn rejects_domain_without_dot() {
    let err = Email::try_new("user@localhost").unwrap_err();
    assert!(err.to_string().contains("at least one '.'"));
}

#[test]
fn rejects_domain_starting_with_dot() {
    let err = Email::try_new("user@.example.com").unwrap_err();
    assert!(err.to_string().contains("domain cannot start or end with '.'"));
}

#[test]
fn rejects_domain_ending_with_dot() {
    let err = Email::try_new("user@example.com.").unwrap_err();
    assert!(err.to_string().contains("domain cannot start or end with '.'"));
}

#[test]
fn rejects_consecutive_dots_in_domain() {
    let err = Email::try_new("user@example..com").unwrap_err();
    assert!(err.to_string().contains("consecutive dots"));
}

#[test]
fn rejects_single_char_tld() {
    let err = Email::try_new("user@example.c").unwrap_err();
    assert!(err.to_string().contains("TLD must be at least 2 characters"));
}

#[test]
fn accepts_two_char_tld() {
    let email = Email::try_new("user@example.uk").unwrap();
    assert_eq!(email.domain(), "example.uk");
}

#[test]
fn local_part_extraction() {
    let email = Email::new("admin@systemprompt.io");
    assert_eq!(email.local_part(), "admin");
}

#[test]
fn domain_extraction() {
    let email = Email::new("admin@systemprompt.io");
    assert_eq!(email.domain(), "systemprompt.io");
}

#[test]
fn display_shows_full_email() {
    let email = Email::new("user@example.com");
    assert_eq!(format!("{}", email), "user@example.com");
}

#[test]
fn serde_roundtrip_exact_json() {
    let email = Email::new("test@example.com");
    let json = serde_json::to_string(&email).unwrap();
    assert_eq!(json, "\"test@example.com\"");
    let deserialized: Email = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, email);
}

#[test]
fn serde_rejects_invalid_email_on_deserialize() {
    let result: Result<Email, _> = serde_json::from_str("\"not-an-email\"");
    assert!(result.is_err());
}

#[test]
fn try_from_str_ref() {
    let email: Email = "valid@example.com".try_into().unwrap();
    assert_eq!(email.as_str(), "valid@example.com");
}

#[test]
fn try_from_string() {
    let email: Email = String::from("valid@example.com").try_into().unwrap();
    assert_eq!(email.as_str(), "valid@example.com");
}

#[test]
fn from_str_parse() {
    let email: Email = "valid@example.com".parse().unwrap();
    assert_eq!(email.as_str(), "valid@example.com");
}

#[test]
fn from_str_parse_rejects_invalid() {
    let result: Result<Email, _> = "invalid".parse();
    assert!(result.is_err());
}

#[test]
fn to_db_value_returns_string_variant() {
    let email = Email::new("user@example.com");
    let db_val = email.to_db_value();
    assert!(matches!(db_val, DbValue::String(s) if s == "user@example.com"));
}

#[test]
fn equality_across_construction_paths() {
    let from_new = Email::new("user@example.com");
    let from_try: Email = "user@example.com".try_into().unwrap();
    let from_parse: Email = "user@example.com".parse().unwrap();
    assert_eq!(from_new, from_try);
    assert_eq!(from_try, from_parse);
}

#[test]
#[should_panic(expected = "Email validation failed")]
fn new_panics_on_invalid() {
    let _ = Email::new("no-at-sign");
}
