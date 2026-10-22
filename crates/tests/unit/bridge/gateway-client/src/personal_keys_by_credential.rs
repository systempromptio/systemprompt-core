//! Personal keys are keyed by credential id: one row per credential, listing
//! every endpoint it serves, and a missing key is named once per credential.

use systemprompt_bridge::gateway::model_view::host_model_view;
use systemprompt_bridge::gateway::personal_keys::personal_key_rows;
use systemprompt_bridge::gateway::types::ProviderHealth;
use systemprompt_models::bridge::profile::ProviderCredentialStatus;
use systemprompt_models::providers::ClientFamily;

fn endpoint(name: &str, credential: &str, configured: bool) -> ProviderHealth {
    ProviderHealth {
        name: name.to_owned(),
        credential: credential.to_owned(),
        clients: vec![ClientFamily::OpenAi],
        configured,
        models: vec![format!("{name}/model")],
        config_issue: None,
        display_name: Some("OpenCode Go".to_owned()),
        personal_key: true,
    }
}

fn status(credential: &str, configured: bool) -> ProviderCredentialStatus {
    ProviderCredentialStatus {
        credential: credential.to_owned(),
        display_name: Some("OpenCode Go".to_owned()),
        configured,
        label: None,
        created_at: None,
        verified_at: None,
        last_used_at: None,
    }
}

fn profile() -> Vec<ProviderHealth> {
    vec![
        endpoint("opencode-go-chat", "opencode-go", false),
        endpoint("opencode-go-anthropic", "opencode-go", false),
        endpoint("deepseek", "deepseek-personal", true),
    ]
}

#[test]
fn one_row_per_credential_with_the_endpoints_it_serves() {
    let rows = personal_key_rows(
        vec![
            status("opencode-go", false),
            status("opencode-go", false),
            status("deepseek-personal", true),
        ],
        &profile(),
    );
    assert_eq!(rows.len(), 2, "OpenCode Go is listed once");
    assert_eq!(rows[0].status.credential, "opencode-go");
    assert_eq!(
        rows[0].endpoints,
        vec![
            "opencode-go-chat".to_owned(),
            "opencode-go-anthropic".to_owned()
        ]
    );
    assert_eq!(rows[1].endpoints, vec!["deepseek".to_owned()]);
}

#[test]
fn a_row_serializes_the_credential_and_endpoints_flat() {
    let rows = personal_key_rows(vec![status("opencode-go", false)], &profile());
    let json = serde_json::to_value(&rows[0]).unwrap();
    assert_eq!(json["credential"], "opencode-go");
    assert_eq!(json["configured"], false);
    assert_eq!(json["endpoints"][1], "opencode-go-anthropic");
}

#[test]
fn a_missing_personal_key_is_named_once_per_credential() {
    let view = host_model_view(&profile(), &[ClientFamily::OpenAi]);
    assert_eq!(view.missing_personal_keys, vec!["OpenCode Go".to_owned()]);
    assert_eq!(view.unconfigured_providers.len(), 2);
    assert_eq!(view.compatible_models, vec!["deepseek/model".to_owned()]);
}
