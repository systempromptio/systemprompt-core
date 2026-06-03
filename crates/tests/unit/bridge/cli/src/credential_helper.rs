use systemprompt_bridge::cli::credential_helper::{error_json, parse_host};

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn parse_host_separate_value() {
    let a = args(&["bin", "credential-helper", "--host", "codex-cli"]);
    assert_eq!(parse_host(&a), Some("codex-cli".to_owned()));
}

#[test]
fn parse_host_equals_form() {
    let a = args(&["bin", "credential-helper", "--host=claude-desktop"]);
    assert_eq!(parse_host(&a), Some("claude-desktop".to_owned()));
}

#[test]
fn parse_host_absent_returns_none() {
    let a = args(&["bin", "credential-helper", "--other", "value"]);
    assert_eq!(parse_host(&a), None);
}

#[test]
fn parse_host_last_arg_without_value_returns_none() {
    let a = args(&["bin", "credential-helper", "--host"]);
    assert_eq!(parse_host(&a), None);
}

#[test]
fn parse_host_only_at_index_one_returns_none() {
    let a = args(&["bin", "--host", "ignored"]);
    assert_eq!(parse_host(&a), None);
}

#[test]
fn error_json_has_error_field() {
    let json = error_json("boom");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value.get("error").and_then(|v| v.as_str()), Some("boom"));
}

#[test]
fn error_json_escapes_special_chars() {
    let msg = r#"he said "hi"
and \backslash\ too"#;
    let json = error_json(msg);
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value.get("error").and_then(|v| v.as_str()), Some(msg));
}
