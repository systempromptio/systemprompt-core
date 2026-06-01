#![allow(clippy::all)]

use systemprompt_config::load_secrets_from_path;

fn valid_secrets_json() -> &'static str {
    r#"{
        "oauth_at_rest_pepper": "this_is_a_long_pepper_value_with_enough_chars_for_validation",
        "database_url": "postgres://user:pass@localhost/db"
    }"#
}

#[test]
fn load_secrets_from_path_succeeds_with_valid_file() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, valid_secrets_json()).unwrap();

    let result = load_secrets_from_path(&path);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
}

#[test]
fn load_secrets_from_path_errors_when_file_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nonexistent.json");

    let err = load_secrets_from_path(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("nonexistent.json") || !msg.is_empty(),
        "got: {msg}"
    );
}

#[test]
fn load_secrets_from_path_errors_on_invalid_json() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, b"{ this is not json }").unwrap();

    let err = load_secrets_from_path(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(!msg.is_empty(), "error message should not be empty");
}

#[test]
fn load_secrets_from_path_errors_on_short_pepper() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    let json =
        r#"{"oauth_at_rest_pepper": "short", "database_url": "postgres://u:p@localhost/db"}"#;
    std::fs::write(&path, json).unwrap();

    let err = load_secrets_from_path(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(!msg.is_empty(), "error message should not be empty");
}

#[test]
fn load_secrets_from_path_errors_on_missing_required_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(
        &path,
        r#"{"oauth_at_rest_pepper": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
    )
    .unwrap();

    let err = load_secrets_from_path(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(!msg.is_empty(), "error message should not be empty");
}

#[test]
fn load_secrets_from_path_strips_null_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    let json = format!(
        r#"{{"oauth_at_rest_pepper": "{}", "database_url": "postgres://u:p@localhost/db", "gemini": null}}"#,
        "a".repeat(32)
    );
    std::fs::write(&path, json).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = systemprompt_config::build_loaded_secrets_message(&secrets);
    assert!(msg.contains("database_url"), "got: {msg}");
    assert!(
        !msg.contains("gemini"),
        "null gemini should not appear: {msg}"
    );
}

#[test]
fn load_secrets_from_path_with_optional_providers() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    let json = format!(
        r#"{{"oauth_at_rest_pepper": "{pepper}", "database_url": "postgres://u:p@localhost/db", "anthropic": "sk-test-key", "github": "ghp_test"}}"#,
        pepper = "a".repeat(32)
    );
    std::fs::write(&path, json).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = systemprompt_config::build_loaded_secrets_message(&secrets);
    assert!(msg.contains("anthropic"), "got: {msg}");
    assert!(msg.contains("github"), "got: {msg}");
}

#[test]
fn load_secrets_from_path_with_write_url() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    let json = format!(
        r#"{{"oauth_at_rest_pepper": "{pepper}", "database_url": "postgres://u:p@localhost/db", "database_write_url": "postgres://u:p@localhost-write/db"}}"#,
        pepper = "a".repeat(32)
    );
    std::fs::write(&path, json).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = systemprompt_config::build_loaded_secrets_message(&secrets);
    assert!(msg.contains("database_write_url"), "got: {msg}");
}

#[test]
fn load_secrets_from_path_errors_on_non_object_json() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, b"[1, 2, 3]").unwrap();

    let err = load_secrets_from_path(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(!msg.is_empty(), "error message should not be empty");
}
