//! Unit tests for secrets_env loading, mapping, and signing-key encoding

use std::collections::HashMap;

use base64::Engine;
use systemprompt_cloud::secrets_env::{
    load_secrets_json, map_secrets_to_env_vars, read_signing_key_pem,
};
use tempfile::TempDir;

fn map_of(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn test_load_secrets_json_keeps_non_empty_strings_only() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("secrets.json");
    std::fs::write(
        &path,
        r#"{"anthropic": "sk-1", "empty": "", "number": 42, "nested": {"a": 1}}"#,
    )
    .unwrap();

    let secrets = load_secrets_json(&path).unwrap();
    assert_eq!(secrets, map_of(&[("anthropic", "sk-1")]));
}

#[test]
fn test_load_secrets_json_non_object_yields_empty_map() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("secrets.json");
    std::fs::write(&path, "[1, 2, 3]").unwrap();

    let secrets = load_secrets_json(&path).unwrap();
    assert!(secrets.is_empty());
}

#[test]
fn test_load_secrets_json_missing_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("secrets.json");
    let err = load_secrets_json(&path).unwrap_err();
    assert_eq!(
        err.to_string(),
        format!("Failed to read {}", path.display())
    );
}

#[test]
fn test_load_secrets_json_invalid_json() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("secrets.json");
    std::fs::write(&path, "not json").unwrap();

    let err = load_secrets_json(&path).unwrap_err();
    assert_eq!(err.to_string(), "Failed to parse secrets.json");
}

#[test]
fn test_map_provider_keys_to_env_var_names() {
    let result = map_secrets_to_env_vars(map_of(&[
        ("gemini", "g"),
        ("anthropic", "a"),
        ("openai", "o"),
        ("database_url", "postgres://db"),
    ]));

    assert_eq!(
        result,
        map_of(&[
            ("GEMINI_API_KEY", "g"),
            ("ANTHROPIC_API_KEY", "a"),
            ("OPENAI_API_KEY", "o"),
            ("DATABASE_URL", "postgres://db"),
        ])
    );
}

#[test]
fn test_internal_database_url_wins_over_external() {
    let result = map_secrets_to_env_vars(map_of(&[
        ("database_url", "postgres://external"),
        ("internal_database_url", "postgres://internal"),
    ]));

    assert_eq!(result, map_of(&[("DATABASE_URL", "postgres://internal")]));
}

#[test]
fn test_custom_keys_advertised_via_custom_secrets() {
    let result = map_secrets_to_env_vars(map_of(&[("my_custom_key", "v")]));

    assert_eq!(result.get("MY_CUSTOM_KEY").map(String::as_str), Some("v"));
    assert_eq!(
        result
            .get("SYSTEMPROMPT_CUSTOM_SECRETS")
            .map(String::as_str),
        Some("MY_CUSTOM_KEY")
    );
}

#[test]
fn test_standard_keys_not_advertised_as_custom() {
    let result = map_secrets_to_env_vars(map_of(&[
        ("anthropic", "a"),
        ("oauth_at_rest_pepper", "p"),
        ("github_token", "t"),
    ]));

    assert!(!result.contains_key("SYSTEMPROMPT_CUSTOM_SECRETS"));
}

#[test]
fn test_system_managed_keys_are_dropped() {
    let result = map_secrets_to_env_vars(map_of(&[("fly_app_name", "x"), ("anthropic", "a")]));

    assert!(!result.contains_key("FLY_APP_NAME"));
    assert_eq!(
        result.get("ANTHROPIC_API_KEY").map(String::as_str),
        Some("a")
    );
}

#[test]
fn test_read_signing_key_pem_missing_returns_none() {
    let temp = TempDir::new().unwrap();
    let result = read_signing_key_pem(&temp.path().join("signing_key.pem")).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_read_signing_key_pem_encodes_base64() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("signing_key.pem");
    let pem = "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----\n";
    std::fs::write(&path, pem).unwrap();

    let encoded = read_signing_key_pem(&path).unwrap().unwrap();
    assert_eq!(
        encoded,
        base64::engine::general_purpose::STANDARD.encode(pem.as_bytes())
    );
}
