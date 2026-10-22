use std::str::FromStr;
use systemprompt_manifest::EnforcementMode;

#[test]
fn display_and_from_str_round_trip() {
    for (variant, text) in [
        (EnforcementMode::Enforce, "enforce"),
        (EnforcementMode::Warn, "warn"),
    ] {
        assert_eq!(variant.to_string(), text);
        assert_eq!(variant.as_str(), text);
        assert_eq!(EnforcementMode::from_str(text).unwrap(), variant);
    }
}

#[test]
fn unknown_and_differently_cased_values_are_errors() {
    for bad in ["warnn", "Warn", "ENFORCE", "", "off"] {
        let err = EnforcementMode::from_str(bad).unwrap_err();
        assert_eq!(err.0, bad);
    }
}

#[test]
fn default_is_enforce_and_only_warn_is_warn() {
    assert_eq!(EnforcementMode::default(), EnforcementMode::Enforce);
    assert!(!EnforcementMode::Enforce.is_warn());
    assert!(EnforcementMode::Warn.is_warn());
}

#[test]
fn serde_uses_lowercase_wire_values() {
    assert_eq!(
        serde_json::to_string(&EnforcementMode::Warn).unwrap(),
        "\"warn\""
    );
    let parsed: EnforcementMode = serde_yaml::from_str("enforce").unwrap();
    assert_eq!(parsed, EnforcementMode::Enforce);
    assert!(serde_yaml::from_str::<EnforcementMode>("Warn").is_err());
}
