//! Which `anthropic-beta` values a forwarded request carries upstream.
//!
//! The accepted list is the endpoint's, not the provider's. First-party
//! Anthropic forwards every beta a client sends. Vertex AI rejects a beta it
//! does not serve, so with no `accepted_betas` a Vertex endpoint forwards
//! none; a declared list forwards only its members, on either host; and
//! `inherit:<endpoint>` forwards exactly the sibling endpoint's list.

use std::collections::BTreeSet;

use serde_json::json;
use systemprompt_ai::UpstreamTarget;
use systemprompt_manifest::services::catalog::{
    BetaSpec, EndpointConfig, ProviderConfig, ResolvedCatalog,
};
use systemprompt_test_fixtures::catalog::{CatalogFixture, claude_model, platform, vertex_regions};
use systemprompt_test_fixtures::keys::test_key;
use systemprompt_wire::WireProtocol;
use systemprompt_wire::anthropic::AnthropicBeta;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const VERTEX_TEMPLATE: &str = "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/publishers/anthropic/models";
const FIRST_PARTY: &str = "https://api.anthropic.com/v1";
const REQUESTED: &str = "context-1m-2025-08-07, interleaved-thinking-2025-05-14";

fn catalog(url: &str, accepted: Option<&[&str]>) -> ResolvedCatalog {
    let endpoint = EndpointConfig {
        accepted_betas: accepted.map(|list| {
            BetaSpec::List(
                list.iter()
                    .map(|b| AnthropicBeta::new(*b))
                    .collect::<BTreeSet<_>>(),
            )
        }),
        ..EndpointConfig::new(WireProtocol::Anthropic, url)
    };
    CatalogFixture::new()
        .credential("claude", platform("claude"))
        .provider("claude", ProviderConfig::new("claude"))
        .endpoint("claude", "messages", endpoint)
        .model("claude-sonnet-5", claude_model(1_000_000))
        .deploy("claude-sonnet-5", ["claude/messages"])
        .build()
}

async fn service_account() -> (MockServer, String) {
    let google = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "access_token": "ya29.betas", "expires_in": 3600 })),
        )
        .mount(&google)
        .await;
    let secret = json!({
        "type": "service_account",
        "project_id": "fixture-project",
        "client_email": "fixture@example.invalid",
        "private_key": test_key(1).to_pkcs8_pem().unwrap(),
        "token_uri": format!("{}/token", google.uri()),
    })
    .to_string();
    (google, secret)
}

async fn forwarded_on(catalog: &ResolvedCatalog, endpoint: &str, secret: &str) -> Option<String> {
    let hop = catalog
        .deployments("claude-sonnet-5")
        .iter()
        .find(|hop| hop.endpoint.as_str() == endpoint)
        .expect("deployed");
    let call = UpstreamTarget::resolve(catalog, hop, secret)
        .expect("resolves")
        .call()
        .await
        .expect("call");
    call.headers_forwarding(
        WireProtocol::Anthropic,
        &[("anthropic-beta".to_owned(), REQUESTED.to_owned())],
    )
    .into_iter()
    .find(|(name, _)| name == "anthropic-beta")
    .map(|(_, value)| value)
}

async fn forwarded_beta(catalog: &ResolvedCatalog, secret: &str) -> Option<String> {
    forwarded_on(catalog, "messages", secret).await
}

#[tokio::test]
async fn first_party_forwards_every_beta_by_default() {
    assert_eq!(
        forwarded_beta(&catalog(FIRST_PARTY, None), "sk-fixture")
            .await
            .as_deref(),
        Some("context-1m-2025-08-07,interleaved-thinking-2025-05-14"),
        "every flag the client sent, rendered in canonical form"
    );
}

#[tokio::test]
async fn a_declared_list_narrows_first_party_too() {
    assert_eq!(
        forwarded_beta(
            &catalog(FIRST_PARTY, Some(&["context-1m-2025-08-07"])),
            "sk-fixture"
        )
        .await
        .as_deref(),
        Some("context-1m-2025-08-07")
    );
}

#[tokio::test]
async fn vertex_forwards_no_beta_unless_declared() {
    let (_google, secret) = service_account().await;
    assert_eq!(
        forwarded_beta(&catalog(VERTEX_TEMPLATE, None), &secret).await,
        None
    );
}

#[tokio::test]
async fn vertex_forwards_only_declared_betas() {
    let (_google, secret) = service_account().await;
    let accepted = catalog(
        VERTEX_TEMPLATE,
        Some(&["context-1m-2025-08-07", "token-efficient-tools-2025-02-19"]),
    );
    assert_eq!(
        forwarded_beta(&accepted, &secret).await.as_deref(),
        Some("context-1m-2025-08-07")
    );

    let unmatched = catalog(VERTEX_TEMPLATE, Some(&["files-api-2025-04-14"]));
    assert_eq!(
        forwarded_beta(&unmatched, &secret).await,
        None,
        "a header left with no accepted beta is not sent at all"
    );
}

#[tokio::test]
async fn an_inheriting_endpoint_forwards_its_siblings_betas() {
    let (_google, secret) = service_account().await;
    let catalog = vertex_regions().build();
    let global = forwarded_on(&catalog, "claude-global", &secret).await;
    let eu = forwarded_on(&catalog, "claude-eu", &secret).await;
    assert_eq!(global.as_deref(), Some("context-1m-2025-08-07"));
    assert_eq!(
        eu, global,
        "inherit:claude-global is the global list, verbatim"
    );
}
