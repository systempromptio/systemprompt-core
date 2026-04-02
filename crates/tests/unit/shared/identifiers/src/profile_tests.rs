use systemprompt_identifiers::{ProfileName, DbValue, ToDbValue};

#[test]
fn valid_alphanumeric_name() {
    let name = ProfileName::try_new("local").unwrap();
    assert_eq!(name.as_str(), "local");
}

#[test]
fn valid_name_with_hyphens() {
    let name = ProfileName::try_new("my-profile").unwrap();
    assert_eq!(name.as_str(), "my-profile");
}

#[test]
fn valid_name_with_underscores() {
    let name = ProfileName::try_new("my_profile").unwrap();
    assert_eq!(name.as_str(), "my_profile");
}

#[test]
fn valid_name_with_digits() {
    let name = ProfileName::try_new("profile123").unwrap();
    assert_eq!(name.as_str(), "profile123");
}

#[test]
fn valid_mixed_name() {
    let name = ProfileName::try_new("my-Profile_2").unwrap();
    assert_eq!(name.as_str(), "my-Profile_2");
}

#[test]
fn rejects_empty_string() {
    let err = ProfileName::try_new("").unwrap_err();
    assert_eq!(err.to_string(), "ProfileName cannot be empty");
}

#[test]
fn rejects_forward_slash() {
    let err = ProfileName::try_new("path/to/profile").unwrap_err();
    assert!(err.to_string().contains("path separator"));
}

#[test]
fn rejects_spaces() {
    let err = ProfileName::try_new("my profile").unwrap_err();
    assert!(err.to_string().contains("alphanumeric"));
}

#[test]
fn rejects_dots() {
    let err = ProfileName::try_new("my.profile").unwrap_err();
    assert!(err.to_string().contains("alphanumeric"));
}

#[test]
fn rejects_at_sign() {
    let err = ProfileName::try_new("user@host").unwrap_err();
    assert!(err.to_string().contains("alphanumeric"));
}

#[test]
fn rejects_exclamation() {
    let err = ProfileName::try_new("profile!").unwrap_err();
    assert!(err.to_string().contains("alphanumeric"));
}

#[test]
fn default_profile_value() {
    let name = ProfileName::default_profile();
    assert_eq!(name.as_str(), "default");
}

#[test]
fn display_shows_name() {
    let name = ProfileName::new("production");
    assert_eq!(format!("{}", name), "production");
}

#[test]
fn serde_roundtrip_exact_json() {
    let name = ProfileName::new("staging");
    let json = serde_json::to_string(&name).unwrap();
    assert_eq!(json, "\"staging\"");
    let deserialized: ProfileName = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, name);
}

#[test]
fn serde_rejects_invalid_on_deserialize() {
    let result: Result<ProfileName, _> = serde_json::from_str("\"has/slash\"");
    assert!(result.is_err());
}

#[test]
fn try_from_str_ref() {
    let name: ProfileName = "local".try_into().unwrap();
    assert_eq!(name.as_str(), "local");
}

#[test]
fn try_from_string() {
    let name: ProfileName = String::from("local").try_into().unwrap();
    assert_eq!(name.as_str(), "local");
}

#[test]
fn from_str_parse() {
    let name: ProfileName = "local".parse().unwrap();
    assert_eq!(name.as_str(), "local");
}

#[test]
fn to_db_value_returns_string_variant() {
    let name = ProfileName::new("local");
    let db_val = name.to_db_value();
    assert!(matches!(db_val, DbValue::String(s) if s == "local"));
}

#[test]
#[should_panic(expected = "ProfileName validation failed")]
fn new_panics_on_invalid() {
    let _ = ProfileName::new("has spaces");
}

#[test]
fn equality_across_construction_paths() {
    let from_new = ProfileName::new("local");
    let from_try: ProfileName = "local".try_into().unwrap();
    let from_parse: ProfileName = "local".parse().unwrap();
    assert_eq!(from_new, from_try);
    assert_eq!(from_try, from_parse);
}
