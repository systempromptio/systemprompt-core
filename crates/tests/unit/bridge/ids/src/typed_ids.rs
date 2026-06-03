use std::str::FromStr;

use systemprompt_bridge::ids::{ModelId, PrefsDomain, PrefsValue};

#[test]
fn try_new_ok_for_non_empty() {
    let domain = PrefsDomain::try_new("editor").expect("non-empty is valid");
    assert_eq!(domain.as_str(), "editor");

    let model = ModelId::try_new("claude-opus").expect("non-empty is valid");
    assert_eq!(model.as_str(), "claude-opus");
}

#[test]
fn try_new_err_for_empty() {
    assert!(PrefsDomain::try_new("").is_err());
    assert!(ModelId::try_new("").is_err());
}

#[test]
fn as_str_into_inner_round_trip() {
    let domain = PrefsDomain::try_new("editor").expect("valid");
    assert_eq!(domain.as_str(), "editor");
    assert_eq!(domain.into_inner(), "editor".to_owned());
}

#[test]
fn try_from_str_and_string() {
    let from_str = PrefsDomain::try_from("editor").expect("valid &str");
    let from_string = PrefsDomain::try_from(String::from("editor")).expect("valid String");
    assert_eq!(from_str, from_string);

    assert!(PrefsDomain::try_from("").is_err());
    assert!(PrefsDomain::try_from(String::new()).is_err());
}

#[test]
fn from_str_trait_validates() {
    let parsed = ModelId::from_str("gpt-5").expect("valid");
    assert_eq!(parsed.as_str(), "gpt-5");
    assert!(ModelId::from_str("").is_err());
}

#[test]
fn serde_round_trip_non_empty_id() {
    let domain = PrefsDomain::try_new("editor").expect("valid");
    let json = serde_json::to_string(&domain).expect("serialize");
    assert_eq!(json, "\"editor\"");
    let back: PrefsDomain = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, domain);
}

#[test]
fn serde_deserialize_empty_into_non_empty_fails() {
    assert!(serde_json::from_str::<PrefsDomain>("\"\"").is_err());
}

#[test]
fn display_equals_as_str_non_empty() {
    let domain = PrefsDomain::try_new("editor").expect("valid");
    assert_eq!(format!("{domain}"), domain.as_str());
}

#[test]
fn plain_id_new_allows_empty() {
    let value = PrefsValue::new("");
    assert_eq!(value.as_str(), "");
}

#[test]
fn plain_id_as_str_into_inner_round_trip() {
    let value = PrefsValue::new("dark");
    assert_eq!(value.as_str(), "dark");
    assert_eq!(value.into_inner(), "dark".to_owned());
}

#[test]
fn plain_id_serde_transparent_round_trip() {
    let value = PrefsValue::new("dark");
    let json = serde_json::to_string(&value).expect("serialize");
    assert_eq!(json, "\"dark\"");
    let back: PrefsValue = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, value);
}

#[test]
fn plain_id_serde_round_trip_empty_string() {
    let empty = serde_json::from_str::<PrefsValue>("\"\"").expect("empty allowed for plain id");
    assert_eq!(empty.as_str(), "");
    let json = serde_json::to_string(&empty).expect("serialize");
    assert_eq!(json, "\"\"");
}

#[test]
fn plain_id_display_equals_as_str() {
    let value = PrefsValue::new("dark");
    assert_eq!(format!("{value}"), value.as_str());
}

#[test]
fn plain_id_from_str_and_string() {
    let owned: PrefsValue = String::from("dark").into();
    let borrowed: PrefsValue = "dark".into();
    assert_eq!(owned, borrowed);
}
