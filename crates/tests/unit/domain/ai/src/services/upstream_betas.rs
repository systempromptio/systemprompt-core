//! Which `anthropic-beta` values a forwarded request carries upstream.
//!
//! First-party Anthropic forwards every beta a client sends. Vertex AI rejects
//! a beta it does not support, so with no `accepted_betas` a Vertex provider
//! forwards none; a declared list forwards only its members, on either host.

use serde_json::json;
use systemprompt_ai::UpstreamTarget;
use systemprompt_identifiers::SecretName;
use systemprompt_models::services::{ProviderEntry, ProviderRegistry, WireProtocol};
use systemprompt_test_fixtures::keys::test_key;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const VERTEX_TEMPLATE: &str =
    "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/publishers/anthropic";
const REQUESTED: &str = "context-1m-2025-08-07, interleaved-thinking-2025-05-14";

fn first_party(accepted: Option<&[&str]>) -> ProviderEntry {
    let mut entry = ProviderRegistry::default_seed()
        .expect("embedded default catalog parses")
        .find_provider("anthropic")
        .expect("anthropic in default catalog")
        .clone();
    entry.accepted_betas = accepted.map(|list| list.iter().map(|b| (*b).to_owned()).collect());
    entry
}

fn vertex(secret: &str, accepted: Option<&[&str]>) -> ProviderEntry {
    let mut entry = first_party(accepted);
    entry.endpoint = VERTEX_TEMPLATE.to_owned();
    entry.api_key_secret = SecretName::new(secret);
    entry
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

async fn forwarded_beta(entry: &ProviderEntry, secret: &str) -> Option<String> {
    let call = UpstreamTarget::resolve(entry, secret)
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

#[tokio::test]
async fn first_party_forwards_every_beta_by_default() {
    assert_eq!(
        forwarded_beta(&first_party(None), "sk-fixture")
            .await
            .as_deref(),
        Some(REQUESTED)
    );
}

#[tokio::test]
async fn a_declared_list_narrows_first_party_too() {
    assert_eq!(
        forwarded_beta(&first_party(Some(&["context-1m-2025-08-07"])), "sk-fixture")
            .await
            .as_deref(),
        Some("context-1m-2025-08-07")
    );
}

#[tokio::test]
async fn vertex_forwards_no_beta_unless_declared() {
    let (_google, secret) = service_account().await;
    assert_eq!(
        forwarded_beta(&vertex("upstream-betas-vertex-none", None), &secret).await,
        None
    );
}

#[tokio::test]
async fn vertex_forwards_only_declared_betas() {
    let (_google, secret) = service_account().await;
    let accepted = vertex(
        "upstream-betas-vertex-some",
        Some(&["context-1m-2025-08-07", "token-efficient-tools-2025-02-19"]),
    );
    assert_eq!(
        forwarded_beta(&accepted, &secret).await.as_deref(),
        Some("context-1m-2025-08-07")
    );

    let unmatched = vertex(
        "upstream-betas-vertex-unmatched",
        Some(&["files-api-2025-04-14"]),
    );
    assert_eq!(
        forwarded_beta(&unmatched, &secret).await,
        None,
        "a header left with no accepted beta is not sent at all"
    );
}
