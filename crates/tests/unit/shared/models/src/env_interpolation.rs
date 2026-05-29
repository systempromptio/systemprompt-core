use std::collections::HashMap;

use systemprompt_models::env::{contains_placeholder, interpolate, read_env_optional};

fn lookup_map<'a>(map: &'a HashMap<&'a str, &'a str>) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| map.get(key).map(|v| v.to_string())
}

#[test]
fn interpolate_replaces_known_variable() {
    let mut map = HashMap::new();
    map.insert("HOST", "localhost");
    let result = interpolate("connect to ${HOST}", &lookup_map(&map));
    assert_eq!(result, "connect to localhost");
}

#[test]
fn interpolate_leaves_unknown_variable_untouched() {
    let map = HashMap::new();
    let result = interpolate("${UNKNOWN}", &lookup_map(&map));
    assert_eq!(result, "${UNKNOWN}");
}

#[test]
fn interpolate_uses_default_when_var_missing() {
    let map = HashMap::new();
    let result = interpolate("${PORT:-8080}", &lookup_map(&map));
    assert_eq!(result, "8080");
}

#[test]
fn interpolate_prefers_lookup_over_default() {
    let mut map = HashMap::new();
    map.insert("PORT", "9090");
    let result = interpolate("${PORT:-8080}", &lookup_map(&map));
    assert_eq!(result, "9090");
}

#[test]
fn interpolate_replaces_multiple_placeholders() {
    let mut map = HashMap::new();
    map.insert("A", "foo");
    map.insert("B", "bar");
    let result = interpolate("${A}-${B}", &lookup_map(&map));
    assert_eq!(result, "foo-bar");
}

#[test]
fn interpolate_no_placeholders_returns_input_unchanged() {
    let map = HashMap::new();
    let input = "plain text no placeholders";
    let result = interpolate(input, &lookup_map(&map));
    assert_eq!(result, input);
}

#[test]
fn interpolate_empty_default_value() {
    let map = HashMap::new();
    let result = interpolate("value:${MISSING:-}", &lookup_map(&map));
    assert_eq!(result, "value:");
}

#[test]
fn contains_placeholder_detects_simple_var() {
    assert!(contains_placeholder("hello ${WORLD}"));
}

#[test]
fn contains_placeholder_detects_default_syntax() {
    assert!(contains_placeholder("${PORT:-3000}"));
}

#[test]
fn contains_placeholder_returns_false_when_absent() {
    assert!(!contains_placeholder("no variables here"));
}

#[test]
fn contains_placeholder_returns_false_for_empty_string() {
    assert!(!contains_placeholder(""));
}

#[test]
fn read_env_optional_returns_none_for_unset_var() {
    unsafe { std::env::remove_var("_TEST_UNSET_VAR_MODELS_12345") };
    let result = read_env_optional("_TEST_UNSET_VAR_MODELS_12345");
    assert!(result.is_none());
}

#[test]
fn read_env_optional_returns_some_for_set_nonempty_var() {
    unsafe { std::env::set_var("_TEST_ENV_OPT_MODELS_99", "hello") };
    let result = read_env_optional("_TEST_ENV_OPT_MODELS_99");
    unsafe { std::env::remove_var("_TEST_ENV_OPT_MODELS_99") };
    assert_eq!(result.as_deref(), Some("hello"));
}

#[test]
fn read_env_optional_treats_empty_string_as_none() {
    unsafe { std::env::set_var("_TEST_ENV_EMPTY_MODELS_99", "") };
    let result = read_env_optional("_TEST_ENV_EMPTY_MODELS_99");
    unsafe { std::env::remove_var("_TEST_ENV_EMPTY_MODELS_99") };
    assert!(result.is_none());
}
