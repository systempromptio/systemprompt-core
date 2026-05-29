use systemprompt_models::services::IncludableString;

#[test]
fn includable_string_inline_from_plain_string() {
    let json = "\"hello world\"";
    let val: IncludableString = serde_json::from_str(json).unwrap();
    assert!(!val.is_include());
    assert_eq!(val.as_inline(), Some("hello world"));
}

#[test]
fn includable_string_include_from_include_prefix() {
    let json = "\"!include ./prompts/system.md\"";
    let val: IncludableString = serde_json::from_str(json).unwrap();
    assert!(val.is_include());
    assert!(val.as_inline().is_none());
}

#[test]
fn includable_string_include_trims_path_whitespace() {
    let json = "\"!include  ./path/with/spaces.md\"";
    let val: IncludableString = serde_json::from_str(json).unwrap();
    assert!(val.is_include());
    if let IncludableString::Include { path } = &val {
        assert_eq!(path, "./path/with/spaces.md");
    } else {
        panic!("expected Include variant");
    }
}

#[test]
fn includable_string_default_is_empty_inline() {
    let val = IncludableString::default();
    assert!(!val.is_include());
    assert_eq!(val.as_inline(), Some(""));
}

#[test]
fn includable_string_serialize_inline() {
    let val = IncludableString::Inline("some text".to_owned());
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, "\"some text\"");
}

#[test]
fn includable_string_inline_as_inline_returns_some() {
    let val = IncludableString::Inline("content".to_owned());
    assert_eq!(val.as_inline(), Some("content"));
}

#[test]
fn includable_string_include_is_include_true() {
    let val = IncludableString::Include {
        path: "some/path.yaml".to_owned(),
    };
    assert!(val.is_include());
    assert!(val.as_inline().is_none());
}

#[test]
fn includable_string_exclamation_without_include_is_inline() {
    let json = "\"!notaninclude directive\"";
    let val: IncludableString = serde_json::from_str(json).unwrap();
    assert!(!val.is_include());
    assert_eq!(val.as_inline(), Some("!notaninclude directive"));
}

#[test]
fn includable_string_include_prefix_only_gives_empty_path() {
    let json = "\"!include \"";
    let val: IncludableString = serde_json::from_str(json).unwrap();
    assert!(val.is_include());
    if let IncludableString::Include { path } = &val {
        assert_eq!(path, "");
    }
}
