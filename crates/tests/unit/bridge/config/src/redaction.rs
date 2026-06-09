use systemprompt_bridge::config::redaction::{is_sensitive_key, redact};

#[test]
fn is_sensitive_key_detects_sensitive_fragments() {
    assert!(is_sensitive_key("secret"));
    assert!(is_sensitive_key("gateway_token"));
    assert!(is_sensitive_key("PAT"), "matching must be case-insensitive");
    assert!(is_sensitive_key("signing_key"));
    assert!(is_sensitive_key("pubkey"));
    assert!(is_sensitive_key("session_id"));
    assert!(is_sensitive_key("password"));
    assert!(is_sensitive_key("credential"));
    assert!(is_sensitive_key("authz"), "contains the 'auth' fragment");
}

#[test]
fn is_sensitive_key_allows_non_sensitive_keys() {
    assert!(!is_sensitive_key("gateway_url"));
    assert!(!is_sensitive_key("host"));
    assert!(!is_sensitive_key("name"));
    assert!(!is_sensitive_key("port"));
}

#[test]
fn redact_replaces_sensitive_values_and_preserves_others() {
    let toml_input = r#"secret = "abc"
gateway_url = "https://gateway.example.com"

[server]
token = "t"
host = "h"

[[items]]
api_key = "key-1"
name = "first"

[[items]]
api_key = "key-2"
name = "second"
"#;
    let mut value: toml::Value = toml::from_str(toml_input).expect("parse toml");
    redact(&mut value);

    assert_eq!(
        value.get("secret").and_then(toml::Value::as_str),
        Some("***REDACTED***"),
    );
    assert_eq!(
        value.get("gateway_url").and_then(toml::Value::as_str),
        Some("https://gateway.example.com"),
    );

    let server = value.get("server").expect("server table present");
    assert_eq!(
        server.get("token").and_then(toml::Value::as_str),
        Some("***REDACTED***"),
    );
    assert_eq!(server.get("host").and_then(toml::Value::as_str), Some("h"),);

    let items = value
        .get("items")
        .and_then(toml::Value::as_array)
        .expect("items array present");
    assert_eq!(items.len(), 2);
    for item in items {
        assert_eq!(
            item.get("api_key").and_then(toml::Value::as_str),
            Some("***REDACTED***"),
        );
    }
    assert_eq!(
        items[0].get("name").and_then(toml::Value::as_str),
        Some("first"),
    );
    assert_eq!(
        items[1].get("name").and_then(toml::Value::as_str),
        Some("second"),
    );
}

#[test]
fn redact_scalar_value_is_noop() {
    let mut value = toml::Value::String("x".to_owned());
    redact(&mut value);
    assert_eq!(value.as_str(), Some("x"));
}
