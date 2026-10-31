//! `admin config migrate` provider folding: grouping, endpoint naming and
//! aborts.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use systemprompt_cli::admin::config::migrate::abort::MigrateAbort;
use systemprompt_cli::admin::config::migrate::fold::fold;
use systemprompt_cli::admin::config::migrate::legacy::{
    LegacyAi, LegacyAiProvider, LegacyProvider,
};
use systemprompt_identifiers::{CredentialId, ProviderId};

fn entries(yaml: &str) -> Vec<LegacyProvider> {
    serde_yaml::from_str(yaml).expect("legacy providers parse")
}

fn ai(pairs: &[(&str, bool, bool)]) -> LegacyAi {
    let mut providers = BTreeMap::new();
    for (name, enabled, search) in pairs {
        providers.insert(
            (*name).to_owned(),
            LegacyAiProvider {
                enabled: *enabled,
                default_model: String::new(),
                default_image_model: String::new(),
                google_search_enabled: *search,
                resilience: None,
            },
        );
    }
    LegacyAi {
        default_provider: None,
        providers,
    }
}

fn abort(yaml: &str, legacy_ai: &LegacyAi) -> MigrateAbort {
    fold(&entries(yaml), legacy_ai).expect_err("fold must abort")
}

#[test]
fn a_lone_entry_folds_to_an_api_endpoint_on_its_secret_credential() {
    let folded = fold(
        &entries(
            "- name: anthropic\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n  api_key_secret: anthropic_key\n",
        ),
        &ai(&[]),
    )
    .unwrap();
    let target = &folded.entries["anthropic"];
    assert_eq!(target.endpoint.endpoint.as_str(), "api");
    assert_eq!(target.credential, CredentialId::new("anthropic_key"));
    let provider = &folded.providers.providers[&ProviderId::new("anthropic")];
    assert!(provider.enabled);
    assert_eq!(provider.endpoints.len(), 1);
    assert!(
        folded
            .credentials
            .credentials
            .contains_key(&CredentialId::new("anthropic_key"))
    );
}

#[test]
fn siblings_sharing_a_secret_fold_into_one_provider_with_wire_and_suffix_endpoints() {
    let folded = fold(
        &entries(
            "- name: gem\n  wire: gemini\n  surface: gemini\n  endpoint: https://g.example\n  api_key_secret: k\n- name: gem-chat\n  wire: openai-chat\n  surface: openai\n  endpoint: https://g.example/v1\n  api_key_secret: k\n",
        ),
        &ai(&[("gem", true, true), ("gem-chat", true, false)]),
    )
    .unwrap();
    assert_eq!(folded.providers.providers.len(), 1);
    assert_eq!(folded.entries["gem"].endpoint.endpoint.as_str(), "gemini");
    assert_eq!(
        folded.entries["gem-chat"].endpoint.endpoint.as_str(),
        "chat"
    );
    let provider = &folded.providers.providers[&ProviderId::new("gem")];
    assert!(provider.google_search_enabled);
    assert_eq!(provider.endpoints.len(), 2);
}

#[test]
fn a_disabled_lone_entry_folds_to_a_disabled_provider() {
    let folded = fold(
        &entries(
            "- name: oai\n  wire: openai-responses\n  surface: openai\n  endpoint: https://o.example\n  api_key_secret: o\n",
        ),
        &ai(&[("oai", false, false)]),
    )
    .unwrap();
    assert!(!folded.providers.providers[&ProviderId::new("oai")].enabled);
}

#[test]
fn a_personal_entry_takes_its_owner_as_credential() {
    let folded = fold(
        &entries(
            "- name: claude-sub\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n  credential: user\n  credential_from: claude\n",
        ),
        &ai(&[]),
    )
    .unwrap();
    assert_eq!(
        folded.entries["claude-sub"].credential,
        CredentialId::new("claude")
    );
}

#[test]
fn a_platform_entry_without_a_secret_aborts_naming_the_key() {
    let err = abort(
        "- name: bare\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n",
        &ai(&[]),
    );
    assert_eq!(err.file, "ai/providers.yaml");
    assert_eq!(err.keys, "providers[bare].api_key_secret");
}

#[test]
fn a_personal_credential_shared_across_unrelated_names_aborts() {
    let err = abort(
        "- name: a\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n  credential: user\n  credential_from: me\n- name: b\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://b.example\n  credential: user\n  credential_from: me\n",
        &ai(&[]),
    );
    assert_eq!(err.keys, "providers[b]");
    assert!(err.reason.contains("personal credential 'me'"));
}

#[test]
fn siblings_disagreeing_on_user_agent_abort_naming_both_keys() {
    let err = abort(
        "- name: p\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n  api_key_secret: k\n  user_agent: one\n- name: p-x\n  wire: openai-chat\n  surface: openai\n  endpoint: https://a.example\n  api_key_secret: k\n  user_agent: two\n",
        &ai(&[]),
    );
    assert_eq!(
        err.keys,
        "providers[p].user_agent, providers[p-x].user_agent"
    );
}

#[test]
fn siblings_disagreeing_on_extra_headers_abort() {
    let err = abort(
        "- name: p\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n  api_key_secret: k\n  extra_headers: {x: '1'}\n- name: p-x\n  wire: openai-chat\n  surface: openai\n  endpoint: https://a.example\n  api_key_secret: k\n",
        &ai(&[]),
    );
    assert!(err.keys.contains("extra_headers"));
}

#[test]
fn siblings_disagreeing_on_enabled_abort_against_the_ai_file() {
    let err = abort(
        "- name: p\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n  api_key_secret: k\n- name: p-x\n  wire: openai-chat\n  surface: openai\n  endpoint: https://a.example\n  api_key_secret: k\n",
        &ai(&[("p", true, false), ("p-x", false, false)]),
    );
    assert_eq!(err.file, "ai/config.yaml");
    assert_eq!(err.keys, "ai.providers.p.enabled, ai.providers.p-x.enabled");
}

#[test]
fn two_siblings_mapping_to_one_endpoint_abort() {
    let err = abort(
        "- name: p\n  wire: openai-chat\n  surface: openai\n  endpoint: https://a.example\n  api_key_secret: k\n- name: p-chat\n  wire: openai-chat\n  surface: openai\n  endpoint: https://a.example\n  api_key_secret: k\n",
        &ai(&[]),
    );
    assert_eq!(err.keys, "providers[p-chat]");
    assert!(err.reason.contains("map to endpoint 'chat'"));
}

#[test]
fn the_abort_message_says_nothing_was_written() {
    let err = abort(
        "- name: bare\n  wire: anthropic\n  surface: anthropic\n  endpoint: https://a.example\n",
        &ai(&[]),
    );
    assert!(err.to_string().ends_with("nothing was written"));
}
