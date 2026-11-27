use systemprompt_bridge::cli::credential_helper::{
    HelperCredential, HelperHostError, credential_for, error_json, parse_host,
};
use systemprompt_bridge::integration::HostKind;
use systemprompt_bridge::proxy::scoped_token::HostOwnsConfig;

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

#[test]
fn a_bridge_configured_host_is_served_its_own_credential() {
    assert_eq!(
        credential_for("claude-desktop"),
        Ok(HelperCredential::GatewayChain)
    );
    assert_eq!(
        credential_for("codex-cli"),
        Ok(HelperCredential::HostToken(HostKind::CodexCli))
    );
    for kind in HostKind::ALL {
        match credential_for(kind.as_str()) {
            Ok(HelperCredential::GatewayChain) => assert_eq!(kind, HostKind::ClaudeDesktop),
            Ok(HelperCredential::HostToken(served)) => assert_eq!(served, kind),
            Err(HelperHostError::HostOwned(HostOwnsConfig(refused))) => assert_eq!(refused, kind),
            Err(HelperHostError::Unknown(err)) => panic!("{kind} is a known host id: {err}"),
        }
    }
}

#[test]
fn a_host_that_owns_its_configuration_is_refused_a_credential() {
    let err = credential_for("harness").expect_err("harness is issued no loopback credential");
    assert_eq!(
        err,
        HelperHostError::HostOwned(HostOwnsConfig(HostKind::Harness))
    );
    assert!(err.to_string().contains("harness"), "{err}");
    assert!(
        err.to_string().contains("no loopback credential"),
        "the refusal says why: {err}"
    );
}

#[test]
fn an_unknown_host_id_is_refused_as_unknown() {
    let err = credential_for("no-such-agent").expect_err("not a host id");
    assert!(matches!(err, HelperHostError::Unknown(_)), "{err:?}");
}
