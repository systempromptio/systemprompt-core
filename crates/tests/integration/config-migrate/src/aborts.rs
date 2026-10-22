//! Every shape `config migrate` refuses to decide, each naming file and keys.

use systemprompt_cli::commands::admin::config::migrate::plan;

use crate::support::{Tree, from_files};

const ALPHA: &str = r"
providers:
  - name: alpha
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1
    api_key_secret: alpha
    models:
      - id: alpha-1
        pricing: { input_per_million: 1.0, output_per_million: 2.0, cache_read_per_million: 0.1 }
";

fn refused(tree: &Tree, file: &str, key: &str) {
    let error = plan(&tree.services).expect_err("migrate must abort");
    let message = format!("{error:#}");
    assert!(
        message.contains(file),
        "abort does not name {file}: {message}"
    );
    assert!(
        message.contains(key),
        "abort does not name {key}: {message}"
    );
    assert!(message.contains("nothing was written"), "{message}");
}

fn gateway(body: &str) -> String {
    format!("gateway:\n  enabled: true\n{body}")
}

#[test]
fn allow_unlisted_models_aborts() {
    let gw = gateway("  default_provider: alpha\n  allow_unlisted_models: true\n");
    refused(
        &from_files(&[("providers.yaml", ALPHA), ("gateway.yaml", &gw)]),
        "ai/gateway.yaml",
        "gateway.allow_unlisted_models",
    );
}

#[test]
fn by_scope_unmapped_deny_aborts() {
    let gw = gateway(
        "  routes:\n    - id: scoped\n      model_pattern: 'alpha-*'\n      provider: alpha\n      by_scope:\n        dimension: project\n        unmapped: deny\n        chains:\n          shop: { provider: alpha }\n",
    );
    refused(
        &from_files(&[("providers.yaml", ALPHA), ("gateway.yaml", &gw)]),
        "ai/gateway.yaml",
        "gateway.routes[scoped].by_scope.unmapped",
    );
}

#[test]
fn route_context_fallbacks_abort() {
    let gw = gateway(
        "  routes:\n    - id: wide\n      model_pattern: 'alpha-*'\n      provider: alpha\n      context_fallbacks:\n        - provider: alpha\n",
    );
    refused(
        &from_files(&[("providers.yaml", ALPHA), ("gateway.yaml", &gw)]),
        "ai/gateway.yaml",
        "gateway.routes[wide]",
    );
}

#[test]
fn route_extra_headers_abort() {
    let gw = gateway(
        "  routes:\n    - id: headed\n      model_pattern: 'alpha-*'\n      provider: alpha\n      extra_headers: { x-team: core }\n",
    );
    refused(
        &from_files(&[("providers.yaml", ALPHA), ("gateway.yaml", &gw)]),
        "ai/gateway.yaml",
        "gateway.routes[headed]",
    );
}

#[test]
fn disagreeing_siblings_abort() {
    let providers = format!(
        "{ALPHA}  - name: alpha-eu\n    wire: anthropic\n    surface: anthropic\n    endpoint: https://eu.example.com/v1\n    api_key_secret: alpha\n    user_agent: other\n    models: []\n"
    );
    refused(
        &from_files(&[("providers.yaml", &providers)]),
        "ai/providers.yaml",
        "providers[alpha-eu].user_agent",
    );
}

#[test]
fn alias_that_is_a_model_id_is_two_price_cards() {
    let providers = format!(
        "{ALPHA}      - id: alpha-2\n        aliases: [alpha-1]\n        pricing: {{ input_per_million: 5.0, output_per_million: 6.0, cache_read_per_million: 0.5 }}\n"
    );
    refused(
        &from_files(&[("providers.yaml", &providers)]),
        "ai/providers.yaml",
        "models[alpha-2].aliases",
    );
}

#[test]
fn personal_credential_across_unrelated_entries_aborts() {
    let providers = r"
providers:
  - name: go
    wire: openai-chat
    surface: openai
    endpoint: https://opencode.ai/v1
    credential: user
    models: []
  - name: other
    wire: anthropic
    surface: openai
    endpoint: https://opencode.ai/anthropic/v1
    credential: user
    credential_from: go
    models: []
";
    refused(
        &from_files(&[("providers.yaml", providers)]),
        "ai/providers.yaml",
        "providers[other]",
    );
}

#[test]
fn override_on_one_endpoint_of_a_folded_provider_aborts() {
    let providers = format!(
        "{ALPHA}  - name: alpha-eu\n    wire: anthropic\n    surface: anthropic\n    endpoint: https://eu.example.com/v1\n    api_key_secret: alpha\n    models: []\n"
    );
    let gw = gateway("  system_prompt_overrides:\n    - provider: alpha-eu\n      action: strip\n");
    refused(
        &from_files(&[("providers.yaml", &providers), ("gateway.yaml", &gw)]),
        "ai/gateway.yaml",
        "gateway.system_prompt_overrides[0].provider",
    );
}

#[test]
fn several_default_image_models_abort() {
    let ai = "ai:\n  providers:\n    alpha: { default_image_model: alpha-1 }\n    beta: { default_image_model: alpha-1 }\n";
    refused(
        &from_files(&[("providers.yaml", ALPHA), ("config.yaml", ai)]),
        "ai/config.yaml",
        "default_image_model",
    );
}

#[test]
fn legacy_section_outside_the_convention_aborts() {
    refused(
        &from_files(&[("extra.yaml", ALPHA)]),
        "extra.yaml",
        "`providers:` as a list of entries",
    );
}
