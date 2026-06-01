//! Tests for `admin::setup::ai_config::apply_ai_defaults` — the in-place
//! reconciliation of `services/ai/config.yaml` with the chosen default
//! provider.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::setup::ai_config::apply_ai_defaults;

fn sample() -> serde_yaml::Value {
    serde_yaml::from_str(
        r"
ai:
  default_provider: anthropic
  providers:
    anthropic: { enabled: true, default_model: claude-sonnet-4-20250514 }
    openai: { enabled: true, default_model: gpt-4-turbo }
    gemini: { enabled: true, default_model: gemini-2.5-flash }
    minimax: { enabled: true, default_model: MiniMax-M2 }
",
    )
    .expect("parse fixture")
}

fn enabled(doc: &serde_yaml::Value, provider: &str) -> Option<bool> {
    doc.get("ai")?
        .get("providers")?
        .get(provider)?
        .get("enabled")?
        .as_bool()
}

#[test]
fn sets_default_and_disables_keyless_standard_providers() {
    let mut doc = sample();
    apply_ai_defaults(&mut doc, "gemini", &["gemini"]).expect("apply");

    assert_eq!(
        doc.get("ai")
            .unwrap()
            .get("default_provider")
            .unwrap()
            .as_str(),
        Some("gemini")
    );
    assert_eq!(enabled(&doc, "gemini"), Some(true));
    assert_eq!(enabled(&doc, "anthropic"), Some(false));
    assert_eq!(enabled(&doc, "openai"), Some(false));
}

#[test]
fn leaves_custom_providers_untouched() {
    let mut doc = sample();
    apply_ai_defaults(&mut doc, "gemini", &["gemini"]).expect("apply");

    let minimax = doc
        .get("ai")
        .unwrap()
        .get("providers")
        .unwrap()
        .get("minimax")
        .unwrap();
    assert_eq!(
        minimax.get("enabled").unwrap().as_bool(),
        Some(true),
        "non-standard providers must keep their enabled flag"
    );
    assert_eq!(
        minimax.get("default_model").unwrap().as_str(),
        Some("MiniMax-M2")
    );
}

#[test]
fn multiple_present_keys_all_stay_enabled() {
    let mut doc = sample();
    apply_ai_defaults(&mut doc, "anthropic", &["anthropic", "openai"]).expect("apply");

    assert_eq!(enabled(&doc, "anthropic"), Some(true));
    assert_eq!(enabled(&doc, "openai"), Some(true));
    assert_eq!(enabled(&doc, "gemini"), Some(false));
}

#[test]
fn errors_when_no_ai_mapping() {
    let mut doc: serde_yaml::Value = serde_yaml::from_str("other: true").unwrap();
    assert!(apply_ai_defaults(&mut doc, "gemini", &["gemini"]).is_err());
}
