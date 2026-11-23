use systemprompt_traits::{SecretPattern, SecretPatternError, is_core_secret_name};

fn pattern(text: &str) -> SecretPattern {
    SecretPattern::parse(text).expect("a valid pattern")
}

#[test]
fn an_exact_pattern_covers_only_its_own_name_byte_for_byte() {
    let exact = pattern("acme_api_key");
    assert!(exact.covers("acme_api_key"));
    assert!(!exact.covers("ACME_API_KEY"));
    assert!(!exact.covers("acme_api_key2"));
}

#[test]
fn a_prefix_covers_a_longer_name_with_the_same_bytes_and_nothing_else() {
    let prefix = pattern("acme_*");
    assert_eq!(prefix, SecretPattern::Prefix("acme_".to_owned()));
    assert!(prefix.covers("acme_x"));
    assert!(!prefix.covers("acmex"));
    assert!(!prefix.covers("ACME_x"));
    assert!(!prefix.covers("acme_"));
}

#[test]
fn a_prefix_grant_covers_an_exact_or_narrower_prefix_declaration_only() {
    let grant = pattern("acme_*");
    assert!(grant.covers_pattern(&pattern("acme_x")));
    assert!(grant.covers_pattern(&pattern("acme_ops_*")));
    assert!(!grant.covers_pattern(&pattern("acm*")));
    assert!(!pattern("acme_x").covers_pattern(&pattern("acme_*")));
    assert!(pattern("acme_x").covers_pattern(&pattern("acme_x")));
}

#[test]
fn an_empty_name_a_bare_wildcard_and_a_misplaced_wildcard_are_refused() {
    assert_eq!(SecretPattern::parse(""), Err(SecretPatternError::Empty));
    assert_eq!(
        SecretPattern::parse("*"),
        Err(SecretPatternError::BareWildcard)
    );
    assert_eq!(
        SecretPattern::prefix(""),
        Err(SecretPatternError::BareWildcard)
    );
    for text in ["*acme", "ac*me", "acme_**", "a*b*"] {
        assert!(
            matches!(
                SecretPattern::parse(text),
                Err(SecretPatternError::MisplacedWildcard(_))
            ),
            "{text}"
        );
    }
}

#[test]
fn a_pattern_round_trips_through_its_text_form() {
    for text in ["acme_api_key", "acme_*"] {
        let parsed: SecretPattern =
            serde_json::from_value(serde_json::Value::String(text.to_owned())).expect("parses");
        assert_eq!(parsed.to_string(), text);
        assert_eq!(serde_json::to_value(&parsed).expect("serializes"), text);
    }
    assert!(
        serde_json::from_value::<SecretPattern>(serde_json::Value::String("*".to_owned())).is_err()
    );
}

#[test]
fn the_core_secret_names_are_matched_without_regard_to_case() {
    for name in [
        "mcp_credential_broker_secret",
        "LICENCE_SIGNING_SECRET",
        "jwt_secret",
        "Encryption_Master_Key",
    ] {
        assert!(is_core_secret_name(name), "{name}");
    }
    assert!(!is_core_secret_name("acme_api_key"));
}
