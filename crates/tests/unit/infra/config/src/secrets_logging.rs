#![allow(clippy::all)]

use systemprompt_config::{build_loaded_secrets_message, load_secrets_from_path};

fn make_secrets(extra: &str) -> impl Fn() -> String + '_ {
    move || {
        format!(
            r#"{{"oauth_at_rest_pepper": "{}", "database_url": "postgres://u:p@localhost/db"{}}}"#,
            "x".repeat(32),
            if extra.is_empty() { String::new() } else { format!(", {extra}") }
        )
    }
}

#[test]
fn build_loaded_secrets_message_contains_base_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, make_secrets("")()).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("oauth_at_rest_pepper"), "got: {msg}");
    assert!(msg.contains("database_url"), "got: {msg}");
    assert!(msg.starts_with("Loaded secrets:"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_no_optional_providers() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, make_secrets("")()).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(!msg.contains("gemini"), "got: {msg}");
    assert!(!msg.contains("anthropic"), "got: {msg}");
    assert!(!msg.contains("openai"), "got: {msg}");
    assert!(!msg.contains("github"), "got: {msg}");
    assert!(!msg.contains("database_write_url"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_includes_gemini_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, make_secrets(r#""gemini": "gemini-key""#)()).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("gemini"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_includes_anthropic_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, make_secrets(r#""anthropic": "sk-ant-test""#)()).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("anthropic"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_includes_openai_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, make_secrets(r#""openai": "sk-openai-test""#)()).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("openai"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_includes_github_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(&path, make_secrets(r#""github": "ghp_test123""#)()).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("github"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_includes_database_write_url_when_set() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(
        &path,
        make_secrets(r#""database_write_url": "postgres://u:p@write-host/db""#)(),
    )
    .unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("database_write_url"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_with_external_database_url() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(
        &path,
        make_secrets(r#""external_database_url": "postgres://u:p@external/db""#)(),
    )
    .unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("external_database_url"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_with_internal_database_url() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    std::fs::write(
        &path,
        make_secrets(r#""internal_database_url": "postgres://u:p@internal/db""#)(),
    )
    .unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("internal_database_url"), "got: {msg}");
}

#[test]
fn build_loaded_secrets_message_all_providers() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("secrets.json");
    let json = format!(
        r#"{{"oauth_at_rest_pepper": "{pepper}", "database_url": "postgres://u:p@localhost/db", "gemini": "g", "anthropic": "a", "openai": "o", "github": "gh"}}"#,
        pepper = "y".repeat(32)
    );
    std::fs::write(&path, json).unwrap();

    let secrets = load_secrets_from_path(&path).unwrap();
    let msg = build_loaded_secrets_message(&secrets);

    assert!(msg.contains("gemini"), "got: {msg}");
    assert!(msg.contains("anthropic"), "got: {msg}");
    assert!(msg.contains("openai"), "got: {msg}");
    assert!(msg.contains("github"), "got: {msg}");
    assert!(!msg.contains("custom"), "no custom keys: {msg}");
}
