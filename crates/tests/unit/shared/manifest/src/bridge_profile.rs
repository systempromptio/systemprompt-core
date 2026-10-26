//! The bridge profile is built from the resolved catalog: servable advertised
//! models, per-provider health keyed by credential, and a `[1m]` default
//! derived from the context window.

use std::collections::BTreeSet;

use systemprompt_catalog_fixtures::{one_platform_provider, opencode_go, vertex_regions};
use systemprompt_manifest::bridge_profile::{
    BridgeProfileParams, CredentialFacts, build, is_model_servable, presented_default_model,
};
use systemprompt_models::providers::ClientFamily;

fn params(catalog: &systemprompt_manifest::services::ResolvedCatalog) -> BridgeProfileParams<'_> {
    BridgeProfileParams {
        inference_gateway_base_url: "https://gw.test/v1".to_owned(),
        auth_scheme: "bearer".to_owned(),
        organization_uuid: None,
        catalog,
    }
}

#[test]
fn the_long_context_marker_is_derived_for_claude_defaults_only() {
    assert_eq!(
        presented_default_model(&one_platform_provider().build()).as_deref(),
        Some("claude-sonnet-5[1m]")
    );
    let short = one_platform_provider()
        .model(
            "claude-small",
            systemprompt_catalog_fixtures::claude_model(200_000),
        )
        .deploy("claude-small", ["anthropic/api"])
        .default_model("claude-small")
        .build();
    assert_eq!(
        presented_default_model(&short).as_deref(),
        Some("claude-small")
    );
}

#[test]
fn a_personal_model_is_servable_only_once_the_member_stored_a_key() {
    let catalog = opencode_go().build();
    let none = BTreeSet::new();
    let no_key = CredentialFacts {
        secret_present: &|_| true,
        personal_keys: &none,
    };
    assert!(!is_model_servable(&catalog, "go-kimi-k3", &no_key));
    let stored: BTreeSet<String> = ["opencode-go".to_owned()].into_iter().collect();
    let with_key = CredentialFacts {
        secret_present: &|_| false,
        personal_keys: &stored,
    };
    assert!(is_model_servable(&catalog, "go-kimi-k3", &with_key));
    assert!(
        is_model_servable(&catalog, "go-minimax-m3", &with_key),
        "one stored key serves both endpoints of the subscription"
    );

    let profile = build(params(&catalog), &no_key);
    assert!(profile.models.is_empty());
    let health = &profile.providers[0];
    assert_eq!(health.credential, "opencode-go");
    assert!(health.personal_key);
    assert!(!health.configured);
    assert_eq!(
        health.clients,
        vec![ClientFamily::Claude, ClientFamily::OpenAi]
    );
    assert_eq!(health.display_name.as_deref(), Some("OpenCode Go"));
}

#[test]
fn the_profile_lists_servable_advertised_models_with_limits() {
    let catalog = vertex_regions().build();
    let none = BTreeSet::new();
    let facts = CredentialFacts {
        secret_present: &|secret| secret == "vertex",
        personal_keys: &none,
    };
    let profile = build(params(&catalog), &facts);
    assert_eq!(
        profile.models,
        vec!["claude-sonnet-5", "vertex-gemini-3.8-flash"]
    );
    assert_eq!(
        profile
            .model_limits
            .get("claude-sonnet-5")
            .map(|l| l.context_window),
        Some(1_000_000)
    );
    assert_eq!(
        profile.default_model.as_deref(),
        Some("claude-sonnet-5[1m]")
    );
    assert!(profile.providers[0].configured);

    let missing = CredentialFacts {
        secret_present: &|_| false,
        personal_keys: &none,
    };
    let profile = build(params(&catalog), &missing);
    assert!(profile.models.is_empty());
    assert_eq!(
        profile.providers[0].config_issue.as_deref(),
        Some("API key secret 'vertex' is not configured")
    );
}

#[test]
fn a_model_with_no_clients_is_never_in_the_profile() {
    let catalog = one_platform_provider().build();
    let none = BTreeSet::new();
    let facts = CredentialFacts {
        secret_present: &|_| true,
        personal_keys: &none,
    };
    let profile = build(params(&catalog), &facts);
    assert_eq!(profile.models, vec!["claude-sonnet-5"]);
    assert_eq!(profile.providers[0].models, vec!["claude-sonnet-5"]);
}
