//! Direct unit coverage of the deploy build-readiness validators in
//! `cloud::tenant::validation` — the AI-config secret derivation branches and
//! the `check_build_ready` missing-release-binary path.

use systemprompt_cli::cloud::tenant::{check_build_ready, validate_ai_config};
use systemprompt_models::{AiProviderConfig, ServicesConfig};

use super::enter;

fn services_with_ai(default: &str, providers: &[(&str, bool)]) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    config.ai.default_provider = default.to_owned();
    for (name, enabled) in providers {
        config.ai.providers.insert(
            (*name).to_owned(),
            AiProviderConfig {
                enabled: *enabled,
                ..AiProviderConfig::default()
            },
        );
    }
    config
}

#[test]
fn validate_ai_config_rejects_empty_default_provider() {
    let config = services_with_ai("", &[]);
    let err = validate_ai_config(&config).expect_err("empty default provider");
    assert!(err.to_string().contains("default_provider"));
}

#[test]
fn validate_ai_config_rejects_unknown_default_provider() {
    let config = services_with_ai("anthropic", &[("openai", true)]);
    let err = validate_ai_config(&config).expect_err("unknown default provider");
    assert!(err.to_string().contains("not found in providers"));
}

#[test]
fn validate_ai_config_rejects_disabled_default_provider() {
    let config = services_with_ai("anthropic", &[("anthropic", false)]);
    let err = validate_ai_config(&config).expect_err("disabled default provider");
    assert!(err.to_string().contains("disabled"));
}

#[test]
fn validate_ai_config_collects_enabled_provider_secrets() {
    let config = services_with_ai(
        "anthropic",
        &[
            ("anthropic", true),
            ("openai", true),
            ("google", true),
            ("custom", true),
        ],
    );
    let secrets = validate_ai_config(&config).expect("valid ai config");
    assert!(secrets.contains(&"ANTHROPIC_API_KEY".to_owned()));
    assert!(secrets.contains(&"OPENAI_API_KEY".to_owned()));
    assert!(secrets.contains(&"GOOGLE_API_KEY".to_owned()));
    assert!(!secrets.iter().any(|s| s.contains("CUSTOM")));
}

#[tokio::test]
async fn check_build_ready_reports_missing_release_binary() {
    let _env = enter().await;
    let err = check_build_ready().expect_err("release binary absent in harness root");
    assert!(!err.is_empty());
}
