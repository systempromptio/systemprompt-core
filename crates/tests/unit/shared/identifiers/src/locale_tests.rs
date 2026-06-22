//! Coverage for the BCP-47-lite `LocaleCode` validator.

use std::str::FromStr;
use systemprompt_identifiers::{DbValue, LocaleCode, ToDbValue};

#[test]
fn accepts_two_letter_primary() {
    assert_eq!(LocaleCode::try_new("en").unwrap().as_str(), "en");
}

#[test]
fn accepts_three_letter_primary() {
    assert!(LocaleCode::try_new("yue").is_ok());
}

#[test]
fn accepts_region_subtag() {
    assert_eq!(LocaleCode::try_new("en-US").unwrap().as_str(), "en-US");
}

#[test]
fn accepts_multiple_subtags() {
    assert!(LocaleCode::try_new("zh-Hant-HK").is_ok());
}

#[test]
fn rejects_empty() {
    assert!(LocaleCode::try_new("").is_err());
}

#[test]
fn rejects_overlong() {
    let too_long = "en-".to_owned() + &"a".repeat(40);
    assert!(LocaleCode::try_new(too_long).is_err());
}

#[test]
fn rejects_single_letter_primary() {
    assert!(LocaleCode::try_new("e").is_err());
}

#[test]
fn rejects_uppercase_primary() {
    assert!(LocaleCode::try_new("EN").is_err());
}

#[test]
fn rejects_one_char_subtag() {
    assert!(LocaleCode::try_new("en-x").is_err());
}

#[test]
fn rejects_non_alphanumeric_subtag() {
    assert!(LocaleCode::try_new("en-U$").is_err());
}

#[test]
fn display_and_as_ref() {
    let code = LocaleCode::try_new("fr-CA").unwrap();
    assert_eq!(format!("{code}"), "fr-CA");
    assert_eq!(AsRef::<str>::as_ref(&code), "fr-CA");
}

#[test]
fn from_str_and_try_from() {
    assert!(LocaleCode::from_str("de").is_ok());
    assert!(LocaleCode::try_from("de-DE".to_owned()).is_ok());
    assert!(LocaleCode::try_from("nope-!").is_err());
}

#[test]
fn serde_roundtrip_and_rejects_invalid() {
    let code = LocaleCode::try_new("pt-BR").unwrap();
    let json = serde_json::to_string(&code).unwrap();
    assert_eq!(json, "\"pt-BR\"");
    let back: LocaleCode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, code);
    assert!(serde_json::from_str::<LocaleCode>("\"E\"").is_err());
}

#[test]
fn to_db_value_is_string() {
    let code = LocaleCode::try_new("ja").unwrap();
    assert!(matches!(code.to_db_value(), DbValue::String(s) if s == "ja"));
}

#[test]
fn new_succeeds_on_valid() {
    assert_eq!(LocaleCode::new("es").as_str(), "es");
}

#[test]
fn to_db_value_via_reference() {
    let code = LocaleCode::try_new("ko").unwrap();
    assert!(matches!((&code).to_db_value(), DbValue::String(s) if s == "ko"));
}
