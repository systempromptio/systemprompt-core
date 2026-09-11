//! Credential triage at the gateway's front door.
//!
//! `authenticate` chooses a verification path from the credential's prefix
//! alone. Every path must refuse an unrecognised credential with 401 and
//! nothing else: a 500 here would tell a caller that a bad token is a server
//! problem, and a 200 would admit an unauthenticated request to a billed
//! upstream. The prefixes must not bleed into one another either — an
//! execution token that fails to verify must not fall through to the JWT
//! decoder and be re-judged there.

use axum::http::StatusCode;
use std::sync::Arc;
use systemprompt_api::routes::gateway::messages::auth::authenticate;
use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
use systemprompt_evaluation::repository::experiments::ExecutionCapabilityRepository;
use systemprompt_identifiers::SessionId;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use systemprompt_traits::AppContext as _;

struct Harness {
    ctx: Arc<AppContext>,
    extractor: JwtContextExtractor,
    capabilities: ExecutionCapabilityRepository,
}

async fn harness() -> Harness {
    let boot = ensure_test_bootstrap();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    let ctx = fixture_app_context(&pool, &boot.database_url).expect("fixture context");
    let extractor = JwtContextExtractor::new(
        ctx.analytics_provider().expect("analytics provider"),
        ctx.user_provider().expect("user provider"),
        JtiRevocationChecker::from_repository(ctx.oauth_repositories().oauth.clone()),
    );
    let capabilities =
        ExecutionCapabilityRepository::new((*pool.write_pool_arc().expect("write pool")).clone());
    Harness {
        ctx,
        extractor,
        capabilities,
    }
}

async fn reject(credential: &str) -> (StatusCode, String) {
    let harness = harness().await;
    authenticate(
        credential,
        &SessionId::generate(),
        &harness.extractor,
        &harness.ctx,
        &harness.capabilities,
    )
    .await
    .expect_err("an unissued credential must never authenticate")
}

#[tokio::test]
async fn an_unissued_execution_token_is_unauthorized_not_a_server_error() {
    let (status, _) = reject("spexec_deadbeef.not-a-real-capability").await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a capability that matches no row is a rejected caller, not a broken server"
    );
}

#[tokio::test]
async fn an_oversized_execution_token_is_refused_without_a_lookup() {
    let (status, _) = reject(&format!("spexec_{}", "a".repeat(200))).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_unknown_api_key_is_unauthorized_and_says_so() {
    let (status, message) = reject("sp-live-0123456789abcdef0123456789abcdef").await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        message.contains("API key"),
        "the caller must be told which credential was rejected: {message}"
    );
}

#[tokio::test]
async fn a_credential_with_no_known_prefix_is_judged_as_a_jwt() {
    let (status, message) = reject("not-a-token").await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an undecodable bearer token is rejected by the JWT path"
    );
    assert!(
        !message.contains("API key"),
        "a plain bearer must not be reported as an API-key failure: {message}"
    );
}

#[tokio::test]
async fn a_structurally_valid_but_unsigned_jwt_is_still_refused() {
    let (status, _) = reject("eyJhbGciOiJub25lIn0.eyJzdWIiOiJhZG1pbiJ9.").await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "an `alg: none` token must not authenticate anyone"
    );
}
