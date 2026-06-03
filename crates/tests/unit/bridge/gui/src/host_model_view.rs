use systemprompt_bridge::auth::types::ProviderHealth;
use systemprompt_bridge::integration::host_app::{HostModelView, host_model_view};
use systemprompt_models::profile::ApiSurface;

fn ph(name: &str, surface: ApiSurface, configured: bool, models: &[&str]) -> ProviderHealth {
    ProviderHealth {
        name: name.to_owned(),
        surface,
        configured,
        models: models.iter().map(|s| (*s).to_owned()).collect(),
        config_issue: (!configured).then(|| "missing".to_owned()),
    }
}

#[test]
fn filters_to_accepted_surface() {
    let health = vec![
        ph("anthropic", ApiSurface::Anthropic, true, &["claude-sonnet-4-6"]),
        ph("gemini", ApiSurface::Gemini, true, &["gemini-3.1-flash-lite-preview"]),
        ph("openai", ApiSurface::OpenAi, true, &["gpt-5"]),
    ];

    assert_eq!(
        host_model_view(&health, &[ApiSurface::Anthropic]).compatible_models,
        vec!["claude-sonnet-4-6".to_owned()]
    );
    assert_eq!(
        host_model_view(&health, &[ApiSurface::OpenAi]).compatible_models,
        vec!["gpt-5".to_owned()]
    );
}

#[test]
fn flags_unconfigured_matching_provider() {
    let health = vec![ph(
        "anthropic",
        ApiSurface::Anthropic,
        false,
        &["claude-sonnet-4-6"],
    )];

    let view = host_model_view(&health, &[ApiSurface::Anthropic]);
    assert!(view.checked);
    assert!(!view.available);
    assert_eq!(view.unconfigured_providers, vec!["anthropic".to_owned()]);
}

#[test]
fn available_only_counts_matching_surface() {
    let health = vec![
        ph("openai", ApiSurface::OpenAi, true, &["gpt-5"]),
        ph("anthropic", ApiSurface::Anthropic, false, &["claude-sonnet-4-6"]),
    ];

    let view = host_model_view(&health, &[ApiSurface::Anthropic]);
    assert!(!view.available);
    assert_eq!(view.compatible_models, vec!["claude-sonnet-4-6".to_owned()]);
}

#[test]
fn unchecked_when_no_health() {
    assert_eq!(
        host_model_view(&[], &[ApiSurface::Anthropic]),
        HostModelView::default()
    );
}
