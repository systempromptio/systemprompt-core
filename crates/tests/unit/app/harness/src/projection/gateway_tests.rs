use serde_json::{Value, json};
use systemprompt_harness::projection::{
    BundleFiles, DEFAULT_MODEL_FAMILIES, Gateway, Protocol, plan,
};
use systemprompt_models::bridge::catalog::ModelCatalog;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::profile::BridgeProfileResponse;

use crate::projection::fixtures;

type Listed<'a> = (&'a str, &'a str, Protocol, Option<u64>, Option<u64>);

fn profile(value: Value) -> BridgeProfileResponse {
    let mut base = json!({
        "inference_gateway_base_url": "http://localhost:8080/v1",
        "auth_scheme": "bearer"
    });
    fixtures::merge(&mut base, value);
    serde_json::from_value(base).unwrap()
}

fn gateway(
    manifest: Value,
    profile: &BridgeProfileResponse,
    catalog: Option<&ModelCatalog>,
    url: &str,
) -> (Gateway, Vec<String>) {
    let manifest = fixtures::manifest(manifest);
    let files = BundleFiles::new();
    let key = fixtures::key();
    let mut input = fixtures::input(&manifest, &files, &key);
    input.profile = Some(profile);
    input.catalog = catalog;
    input.instance_url = url;
    let projection = plan(&input);
    (projection.gateway, projection.warnings)
}

fn routes(gateway: &Gateway) -> Vec<(&str, Protocol)> {
    gateway
        .models
        .iter()
        .map(|m| (m.id.as_str(), m.protocol))
        .collect()
}

fn alpha_profile() -> BridgeProfileResponse {
    profile(json!({
        "models": ["claude-haiku-5-5", "claude-opus-5-5", "claude-sonnet-5-5", "go-kimi-k3"],
        "default_model": "claude-sonnet-5-5[1m]",
        "providers": [
            { "name": "anthropic", "credential": "anthropic", "clients": ["claude"], "configured": true,
              "models": ["claude-haiku-5-5", "claude-opus-5-5", "claude-sonnet-5-5"] },
            { "name": "go", "credential": "go", "clients": ["openai"], "configured": false,
              "models": ["go-kimi-k3"] }
        ],
        "model_limits": {
            "claude-sonnet-5-5": { "context_window": 1_000_000, "max_output_tokens": 128_000 }
        }
    }))
}

#[test]
fn claude_models_are_listed_on_messages_when_the_host_admits_claude() {
    let catalog: ModelCatalog = serde_json::from_value(json!({ "data": [
        { "id": "claude-haiku-5-5", "display_name": "Claude Haiku 5.5",
          "max_input_tokens": 200_000, "max_tokens": 64_000 },
        { "id": "claude-sonnet-5-5", "display_name": "Claude Sonnet 5.5" },
        { "id": "go-kimi-k3" }
    ] }))
    .unwrap();
    let profile = alpha_profile();
    let (gateway, warnings) = gateway(
        json!({ "host_model_protocols": { HostKind::Harness.as_str(): ["claude", "openai"] } }),
        &profile,
        Some(&catalog),
        "https://sp.example.com/",
    );
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(
        gateway.base_url.as_deref(),
        Some("https://sp.example.com/v1")
    );
    assert_eq!(gateway.default_model.as_deref(), Some("claude-sonnet-5-5"));
    assert_eq!(gateway.protocols, ["claude", "openai"]);
    let listed: Vec<Listed<'_>> = gateway
        .models
        .iter()
        .map(|m| {
            (
                m.id.as_str(),
                m.name.as_str(),
                m.protocol,
                m.context,
                m.output,
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (
                "claude-haiku-5-5",
                "Claude Haiku 5.5",
                Protocol::Messages,
                Some(200_000),
                Some(64_000)
            ),
            (
                "claude-sonnet-5-5",
                "Claude Sonnet 5.5",
                Protocol::Messages,
                Some(1_000_000),
                Some(128_000)
            ),
        ]
    );
    assert_eq!(gateway.models[0].families, ["claude"]);
}

#[test]
fn without_an_entry_the_host_lists_both_families() {
    assert_eq!(DEFAULT_MODEL_FAMILIES, ["claude", "openai"]);
    let profile = alpha_profile();
    let (gateway, warnings) = gateway(json!({}), &profile, None, "https://sp.example.com");
    assert_eq!(warnings, Vec::<String>::new());
    assert_eq!(gateway.protocols, DEFAULT_MODEL_FAMILIES);
    assert_eq!(
        routes(&gateway),
        [
            ("claude-haiku-5-5", Protocol::Messages),
            ("claude-opus-5-5", Protocol::Messages),
            ("claude-sonnet-5-5", Protocol::Messages),
        ]
    );
    assert_eq!(gateway.default_model.as_deref(), Some("claude-sonnet-5-5"));
    let openai = profile_with_openai();
    let (gateway, _) = gateway_for(&openai);
    assert_eq!(routes(&gateway), [("gpt-x", Protocol::ChatCompletions)]);
}

fn profile_with_openai() -> BridgeProfileResponse {
    profile(json!({
        "models": ["gpt-x"],
        "default_model": "gpt-x",
        "providers": [
            { "name": "openai", "credential": "o", "clients": ["openai"], "configured": true,
              "models": ["gpt-x"] }
        ]
    }))
}

fn gateway_for(profile: &BridgeProfileResponse) -> (Gateway, Vec<String>) {
    gateway(json!({}), profile, None, "https://sp.example.com")
}

#[test]
fn openai_models_go_to_chat_completions_and_odd_entries_are_dropped() {
    let profile = profile(json!({
        "models": ["gpt-x", "dup", "dup", "off", "bad id", "gemini-only", "both"],
        "default_model": "nope",
        "providers": [
            { "name": "openai", "credential": "o", "clients": ["openai"], "configured": true,
              "models": ["gpt-x"] },
            { "name": "deepseek", "credential": "d", "configured": false, "models": ["off"] },
            { "name": "other", "credential": "g", "clients": ["gemini"], "configured": true,
              "models": ["gemini-only"] },
            { "name": "direct", "credential": "x", "clients": ["openai"], "configured": false,
              "models": ["both"] },
            { "name": "bedrock", "credential": "b", "clients": ["claude", "openai"],
              "configured": true, "models": ["both"] }
        ]
    }));
    let (gateway, warnings) = gateway(
        json!({ "host_model_protocols": { HostKind::Harness.as_str(): ["openai", "claude"] } }),
        &profile,
        None,
        "http://127.0.0.1:18101",
    );
    assert_eq!(
        routes(&gateway),
        [
            ("gpt-x", Protocol::ChatCompletions),
            ("dup", Protocol::Messages),
            ("both", Protocol::Messages)
        ]
    );
    assert_eq!(gateway.models[1].name, "dup");
    assert_eq!(warnings.len(), 1);
    assert_eq!(gateway.default_model, None);
    assert_eq!(
        gateway.base_url.as_deref(),
        Some("http://127.0.0.1:18101/v1")
    );
}

#[test]
fn an_entry_without_claude_hides_claude_only_models() {
    let profile = alpha_profile();
    let (gateway, _) = gateway(
        json!({ "host_model_protocols": { HostKind::Harness.as_str(): ["openai"] } }),
        &profile,
        None,
        "https://sp.example.com",
    );
    assert_eq!(
        gateway.models,
        Vec::<systemprompt_harness::projection::GatewayModel>::new()
    );
    assert_eq!(gateway.default_model, None);
    assert_eq!(gateway.protocols, ["openai"]);
}

#[test]
fn an_insecure_instance_url_lists_nothing() {
    let profile = alpha_profile();
    let (gateway, warnings) = gateway(
        json!({ "host_model_protocols": { HostKind::Harness.as_str(): ["claude"] } }),
        &profile,
        None,
        "http://sp.example.com",
    );
    assert_eq!(
        gateway.models,
        Vec::<systemprompt_harness::projection::GatewayModel>::new()
    );
    assert_eq!(gateway.base_url, None);
    assert_eq!(warnings.len(), 1);
}
