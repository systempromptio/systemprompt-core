use systemprompt_bridge::integration::claude_desktop::reg_profile::{
    parse_reg_entries, profile_entries, render_reg,
};
use systemprompt_bridge::integration::host_app::ProfileGenInputs;

fn inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        gateway_base_url: "https://gateway.systemprompt.io".to_string(),
        api_key: "sp-secret-key".to_string(),
        models: vec!["claude-opus-4-7".to_string()],
        organization_uuid: Some("org-abc".to_string()),
    }
}

fn value_of<'a>(entries: &'a [(String, String)], name: &str) -> &'a str {
    entries
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("missing {name} in {entries:?}"))
}

#[test]
fn profile_entries_carry_required_policy_keys() {
    let entries = profile_entries(&inputs());
    let owned: Vec<(String, String)> = entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    assert_eq!(value_of(&owned, "inferenceProvider"), "gateway");
    assert_eq!(value_of(&owned, "inferenceGatewayAuthScheme"), "bearer");
    assert_eq!(
        value_of(&owned, "inferenceGatewayBaseUrl"),
        "https://gateway.systemprompt.io"
    );
    assert_eq!(value_of(&owned, "inferenceGatewayApiKey"), "sp-secret-key");
    assert_eq!(value_of(&owned, "inferenceModels"), "[\"claude-opus-4-7\"]");
}

#[test]
fn empty_models_falls_back_to_defaults() {
    let mut probe = inputs();
    probe.models = vec![];
    let entries: Vec<(String, String)> = profile_entries(&probe)
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    let parsed: Vec<String> = serde_json::from_str(value_of(&entries, "inferenceModels"))
        .expect("models is a json array");
    assert!(
        parsed.len() >= 2,
        "expected default model list, got {parsed:?}"
    );
    assert!(parsed.iter().any(|m| m == "claude-opus-4-7"));
}

#[test]
fn render_targets_hkcu_unelevated_and_hklm_elevated() {
    assert!(render_reg(false, &inputs()).contains(r"[HKEY_CURRENT_USER\SOFTWARE\Policies\Claude]"));
    assert!(render_reg(true, &inputs()).contains(r"[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Claude]"));
}

#[test]
fn hklm_profile_parses_to_all_five_policy_values() {
    let rendered = render_reg(true, &inputs());
    assert!(rendered.contains(r"[HKEY_LOCAL_MACHINE\SOFTWARE\Policies\Claude]"));
    let parsed = parse_reg_entries(&rendered);
    let names: Vec<&str> = parsed.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "inferenceProvider",
            "inferenceGatewayBaseUrl",
            "inferenceGatewayApiKey",
            "inferenceGatewayAuthScheme",
            "inferenceModels",
        ]
    );
}

#[test]
fn rendered_profile_round_trips_through_parser() {
    let probe = inputs();
    let rendered = render_reg(false, &probe);
    assert!(rendered.starts_with("Windows Registry Editor Version 5.00"));

    let parsed = parse_reg_entries(&rendered);
    let expected: Vec<(String, String)> = profile_entries(&probe)
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    assert_eq!(parsed, expected);
}

#[test]
fn round_trip_preserves_backslashes_and_quotes() {
    let mut probe = inputs();
    probe.api_key = r#"key-with-"quote"-and-\back\slash"#.to_string();
    let parsed = parse_reg_entries(&render_reg(false, &probe));
    assert_eq!(
        value_of(&parsed, "inferenceGatewayApiKey"),
        r#"key-with-"quote"-and-\back\slash"#
    );
}

#[test]
fn parser_ignores_header_and_section_lines() {
    let parsed = parse_reg_entries(
        "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\SOFTWARE\\Policies\\Claude]\r\n\"inferenceProvider\"=\"gateway\"\r\n",
    );
    assert_eq!(
        parsed,
        vec![("inferenceProvider".to_string(), "gateway".to_string())]
    );
}
