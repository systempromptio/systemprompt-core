use systemprompt_bridge::auth::types::ProviderHealth;
use systemprompt_bridge::integration::host_app::{HostModelView, host_model_view};

fn ph(name: &str, protocol: &str, configured: bool, models: &[&str]) -> ProviderHealth {
    ProviderHealth {
        name: name.to_owned(),
        protocol: protocol.to_owned(),
        configured,
        models: models.iter().map(|s| (*s).to_owned()).collect(),
        config_issue: (!configured).then(|| "missing".to_owned()),
    }
}

#[test]
fn filters_to_accepted_protocol() {
    let health = vec![
        ph("anthropic", "anthropic", true, &["claude-sonnet-4-6"]),
        ph("gemini", "gemini", true, &["gemini-3.1-flash-lite-preview"]),
        ph("openai", "openai-responses", true, &["gpt-5"]),
    ];

    assert_eq!(
        host_model_view(&health, &["anthropic"]).compatible_models,
        vec!["claude-sonnet-4-6".to_owned()]
    );
    assert_eq!(
        host_model_view(&health, &["openai-chat", "openai-responses"]).compatible_models,
        vec!["gpt-5".to_owned()]
    );
}

#[test]
fn flags_unconfigured_matching_provider() {
    let health = vec![ph("anthropic", "anthropic", false, &["claude-sonnet-4-6"])];

    let view = host_model_view(&health, &["anthropic"]);
    assert!(view.checked);
    assert!(!view.available);
    assert_eq!(view.unconfigured_providers, vec!["anthropic".to_owned()]);
}

#[test]
fn available_only_counts_matching_protocol() {
    let health = vec![
        ph("openai", "openai-responses", true, &["gpt-5"]),
        ph("anthropic", "anthropic", false, &["claude-sonnet-4-6"]),
    ];

    let view = host_model_view(&health, &["anthropic"]);
    assert!(!view.available);
    assert_eq!(view.compatible_models, vec!["claude-sonnet-4-6".to_owned()]);
}

#[test]
fn unchecked_when_no_health() {
    assert_eq!(
        host_model_view(&[], &["anthropic"]),
        HostModelView::default()
    );
}
