//! Tests for the interactive provider-key prompts in `admin setup`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::admin::setup::secrets::SecretsData;
use systemprompt_cli::admin::setup::secrets::prompts::{
    resolve_interactive_primary, select_provider_keys,
};
use systemprompt_identifiers::ProviderId;

#[test]
fn selecting_single_provider_stores_key_and_returns_default() {
    let cases = [(0usize, "gemini"), (1, "anthropic"), (2, "openai")];
    for (index, provider) in cases {
        let prompter = ScriptedPrompter::new([index.to_string(), "sk-key".to_owned()]);
        let mut secrets = SecretsData::default();
        let picked = select_provider_keys(&prompter, &mut secrets).unwrap();
        assert_eq!(picked, Some(ProviderId::new(provider)));
        let stored = match provider {
            "gemini" => &secrets.gemini,
            "anthropic" => &secrets.anthropic,
            _ => &secrets.openai,
        };
        assert_eq!(stored.as_deref(), Some("sk-key"));
    }
}

#[test]
fn empty_key_for_single_provider_is_rejected() {
    let prompter = ScriptedPrompter::new(["1", ""]);
    let mut secrets = SecretsData::default();
    let err = select_provider_keys(&prompter, &mut secrets).unwrap_err();
    assert!(err.to_string().contains("API key is required"));
    assert!(secrets.anthropic.is_none());
}

#[test]
fn multiple_keys_path_skips_blank_entries_and_returns_no_default() {
    let prompter = ScriptedPrompter::new(["3", "", "sk-anthropic", "", "gh-token"]);
    let mut secrets = SecretsData::default();

    let picked = select_provider_keys(&prompter, &mut secrets).unwrap();

    assert_eq!(picked, None);
    assert!(secrets.gemini.is_none());
    assert_eq!(secrets.anthropic.as_deref(), Some("sk-anthropic"));
    assert!(secrets.openai.is_none());
    assert_eq!(secrets.github.as_deref(), Some("gh-token"));
}

#[test]
fn resolve_primary_keeps_explicit_choice() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let secrets = SecretsData::default();
    let explicit = Some(ProviderId::new("openai"));

    let resolved = resolve_interactive_primary(&prompter, explicit.clone(), &secrets).unwrap();

    assert_eq!(resolved, explicit);
}

#[test]
fn resolve_primary_returns_none_without_keys_and_sole_provider_without_prompting() {
    let prompter = ScriptedPrompter::new(Vec::<String>::new());
    let secrets = SecretsData::default();
    assert_eq!(
        resolve_interactive_primary(&prompter, None, &secrets).unwrap(),
        None
    );

    let mut secrets = SecretsData::default();
    secrets.openai = Some("sk".to_owned());
    assert_eq!(
        resolve_interactive_primary(&prompter, None, &secrets).unwrap(),
        Some(ProviderId::new("openai"))
    );
}

#[test]
fn resolve_primary_prompts_between_multiple_present_providers() {
    let prompter = ScriptedPrompter::new(["1"]);
    let mut secrets = SecretsData::default();
    secrets.gemini = Some("g".to_owned());
    secrets.anthropic = Some("a".to_owned());

    let resolved = resolve_interactive_primary(&prompter, None, &secrets).unwrap();

    assert_eq!(resolved, Some(ProviderId::new("anthropic")));
}
