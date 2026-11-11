use std::collections::HashMap;

use serde_yaml::Value;
use systemprompt_manifest::env::{
    FieldKind, InterpolationError, PathSegment, interpolate_yaml, untyped,
};
use systemprompt_manifest::profile::profile_field_kind;

fn resolve(yaml: &str, vars: &[(&str, &str)]) -> Result<Value, InterpolationError> {
    let map: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let mut value: Value = serde_yaml::from_str(yaml).expect("fixture parses");
    interpolate_yaml(&mut value, &|name| map.get(name).cloned(), &untyped)?;
    Ok(value)
}

fn get<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter()
        .fold(value, |v, key| v.get(*key).expect("key present"))
}

#[test]
fn a_value_with_comment_and_colon_is_kept_whole() {
    let value = resolve(
        "database:\n  password: ${DB_PASS}\n  user: app\n",
        &[("DB_PASS", "p#ss: x")],
    )
    .expect("resolves");
    assert_eq!(
        get(&value, &["database", "password"]),
        &Value::String("p#ss: x".to_owned())
    );
}

#[test]
fn a_value_with_a_newline_cannot_inject_a_sibling_key() {
    let value = resolve(
        "security:\n  token: ${TOKEN}\n",
        &[("TOKEN", "abc\n  trusted_proxies: [0.0.0.0/0]")],
    )
    .expect("resolves");
    let security = get(&value, &["security"]).as_mapping().expect("mapping");
    assert_eq!(security.len(), 1);
    assert!(security.get("trusted_proxies").is_none());
    assert_eq!(
        get(&value, &["security", "token"]),
        &Value::String("abc\n  trusted_proxies: [0.0.0.0/0]".to_owned())
    );
}

#[test]
fn a_flow_mapping_value_stays_a_string() {
    let value = resolve("key: ${V}\n", &[("V", "{a: 1}")]).expect("resolves");
    assert_eq!(get(&value, &["key"]), &Value::String("{a: 1}".to_owned()));
}

#[test]
fn a_default_with_metacharacters_is_kept_whole() {
    let value = resolve("key: \"${MISSING:-a # b}\"\n", &[]).expect("resolves");
    assert_eq!(get(&value, &["key"]), &Value::String("a # b".to_owned()));
}

#[test]
fn an_unset_variable_is_an_error_naming_variable_and_key_path() {
    let error = resolve(
        "server:\n  hosts:\n    - ok\n    - http://${NOT_SET_HOST}:8080\n",
        &[],
    )
    .expect_err("unset must fail");
    assert_eq!(error.variable, "NOT_SET_HOST");
    assert_eq!(error.key_path, "server.hosts[1]");
    assert!(error.to_string().contains("NOT_SET_HOST"));
}

fn resolve_profile_typed(yaml: &str, vars: &[(&str, &str)]) -> Value {
    let map: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let mut value: Value = serde_yaml::from_str(yaml).expect("fixture parses");
    interpolate_yaml(
        &mut value,
        &|name| map.get(name).cloned(),
        &profile_field_kind,
    )
    .expect("resolves");
    value
}

#[test]
fn a_numeric_field_receives_a_number_and_a_string_field_keeps_text() {
    let value = resolve_profile_typed(
        "server:\n  port: ${PORT}\n  host: ${HOST}\nsite:\n  name: ${NAME}\n",
        &[("PORT", "8080"), ("HOST", "0012"), ("NAME", "12345")],
    );
    assert_eq!(
        get(&value, &["server", "port"]),
        &Value::Number(8080.into())
    );
    assert_eq!(
        get(&value, &["server", "host"]),
        &Value::String("0012".to_owned())
    );
    assert_eq!(
        get(&value, &["site", "name"]),
        &Value::String("12345".to_owned())
    );
}

#[test]
fn a_string_field_keeps_values_that_look_like_other_types() {
    for raw in ["true", "null", "0x1F", "1e5", "007", "~"] {
        let value = resolve_profile_typed("site:\n  name: ${NAME}\n", &[("NAME", raw)]);
        assert_eq!(
            get(&value, &["site", "name"]),
            &Value::String(raw.to_owned()),
            "{raw}"
        );
    }
}

#[test]
fn a_boolean_field_receives_a_boolean() {
    let value = resolve_profile_typed("rate_limits:\n  disabled: ${OFF}\n", &[("OFF", "true")]);
    assert_eq!(
        get(&value, &["rate_limits", "disabled"]),
        &Value::Bool(true)
    );
}

#[test]
fn an_empty_default_is_null_only_where_the_field_accepts_null() {
    let value = resolve_profile_typed(
        "server:\n  metrics_port: ${METRICS:-}\nsite:\n  name: ${NAME:-}\n",
        &[],
    );
    assert_eq!(get(&value, &["server", "metrics_port"]), &Value::Null);
    assert_eq!(
        get(&value, &["site", "name"]),
        &Value::String(String::new())
    );
}

#[test]
fn an_untyped_path_keeps_the_value_as_text() {
    let value = resolve("server:\n  port: ${PORT}\n", &[("PORT", "8080")]).expect("resolves");
    assert_eq!(
        get(&value, &["server", "port"]),
        &Value::String("8080".to_owned())
    );
}

#[test]
fn the_profile_schema_types_known_paths() {
    let key = |k: &str| PathSegment::Key(k.to_owned());
    assert!(profile_field_kind(&[key("server"), key("port")]).accepts(FieldKind::NUMBER));
    assert!(!profile_field_kind(&[key("server"), key("port")]).accepts_string());
    assert!(profile_field_kind(&[key("site"), key("name")]).accepts_string());
    assert!(profile_field_kind(&[key("server"), key("metrics_port")]).accepts(FieldKind::NULL));
}

#[test]
fn an_embedded_placeholder_stays_a_string() {
    let value = resolve(
        "url: http://${HOST}:${PORT}\n",
        &[("HOST", "h"), ("PORT", "1")],
    )
    .expect("resolves");
    assert_eq!(
        get(&value, &["url"]),
        &Value::String("http://h:1".to_owned())
    );
}

#[test]
fn mapping_keys_are_not_interpolated() {
    let value = resolve("\"${K}\": v\n", &[]).expect("keys are left alone");
    assert_eq!(value.get("${K}"), Some(&Value::String("v".to_owned())));
}
