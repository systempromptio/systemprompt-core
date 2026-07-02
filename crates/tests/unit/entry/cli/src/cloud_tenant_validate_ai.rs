//! Unit tests for `cloud::tenant::validate_ai_config`.
//!
//! The validator checks that the AI block names an enabled default provider and
//! projects the set of secret env-var names every enabled provider requires.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;
use systemprompt_cli::cloud::tenant::validate_ai_config;
use systemprompt_models::{AiConfig, AiProviderConfig, ServicesConfig};

fn provider(enabled: bool) -> AiProviderConfig {
    AiProviderConfig {
        enabled,
        ..AiProviderConfig::default()
    }
}

fn services_with(default_provider: &str, providers: &[(&str, bool)]) -> ServicesConfig {
    let mut map = HashMap::new();
    for (name, enabled) in providers {
        map.insert((*name).to_owned(), provider(*enabled));
    }
    ServicesConfig {
        ai: AiConfig {
            default_provider: default_provider.to_owned(),
            providers: map,
            ..AiConfig::default()
        },
        ..ServicesConfig::default()
    }
}

#[test]
fn missing_default_provider_is_rejected() {
    let cfg = services_with("", &[]);
    let err = validate_ai_config(&cfg).unwrap_err();
    assert!(err.to_string().contains("default_provider"));
}

#[test]
fn unknown_default_provider_is_rejected() {
    let cfg = services_with("anthropic", &[("openai", true)]);
    let err = validate_ai_config(&cfg).unwrap_err();
    assert!(err.to_string().contains("not found in providers"));
}

#[test]
fn disabled_default_provider_is_rejected() {
    let cfg = services_with("anthropic", &[("anthropic", false)]);
    let err = validate_ai_config(&cfg).unwrap_err();
    assert!(err.to_string().contains("disabled"));
}

#[test]
fn enabled_providers_project_their_secret_keys() {
    let cfg = services_with(
        "anthropic",
        &[("anthropic", true), ("openai", true), ("google", true)],
    );
    let mut secrets = validate_ai_config(&cfg).expect("valid config");
    secrets.sort();
    assert_eq!(
        secrets,
        vec![
            "ANTHROPIC_API_KEY".to_owned(),
            "GOOGLE_API_KEY".to_owned(),
            "OPENAI_API_KEY".to_owned(),
        ]
    );
}

#[test]
fn disabled_and_unmapped_providers_are_skipped() {
    let cfg = services_with(
        "anthropic",
        &[("anthropic", true), ("openai", false), ("gemini", true)],
    );
    let secrets = validate_ai_config(&cfg).expect("valid config");
    assert_eq!(secrets, vec!["ANTHROPIC_API_KEY".to_owned()]);
}
