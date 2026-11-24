//! Credential triage at the gateway's front door.
//!
//! `authenticate` chooses a verification path from the credential's prefix
//! alone. Every path must refuse an unrecognised credential with 401 and
//! nothing else: a 500 here would tell a caller that a bad token is a server
//! problem, and a 200 would admit an unauthenticated request to a billed
//! upstream. The prefixes must not bleed into one another either — an API
//! key that fails to verify must not fall through to the JWT decoder and be
//! re-judged there.

use axum::http::StatusCode;
use systemprompt_gateway_entry::http::messages::auth::{AuthInputs, authenticate};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_identifiers::SessionId;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::AppContextSeeds as _;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context};

struct Harness {
    state: systemprompt_gateway_entry::http::state::GatewayHttpState,
}

fn gateway_http_state(
    ctx: &AppContext,
) -> systemprompt_gateway_entry::http::state::GatewayHttpState {
    let state = ctx.kernel().state();
    systemprompt_gateway_entry::http::state::GatewayHttpState {
        authenticator: state
            .get::<systemprompt_models::auth::DynRequestAuthenticator>()
            .cloned()
            .expect("request authenticator"),
        api_keys: systemprompt_gateway_entry::http::state::GatewayApiKeys::new(
            systemprompt_users::ApiKeyService::new(
                systemprompt_test_fixtures::user_repository(&ctx),
                systemprompt_users::ApiKeyLifetime::from_max_days(
                    ctx.config().api_key_max_lifetime_days,
                ),
            ),
        ),
        session_mint: state
            .get::<systemprompt_traits::DynGatewaySessionMint>()
            .cloned()
            .expect("gateway session mint"),
        repos: std::sync::Arc::new(
            systemprompt_gateway::surface::repositories::gateway_repositories(
                &ctx.kernel().resolved(),
                &systemprompt_api::routes::gateway::host_handles(ctx).expect("host handles"),
                ctx.kernel()
                    .resolved()
                    .get::<systemprompt_traits::DynContextMaterializer>()
                    .expect("context materializer"),
                ctx.db_pool(),
            )
            .expect("gateway repositories"),
        ),
        engines: systemprompt_api::routes::gateway::gateway_engines(ctx.kernel())
            .expect("gateway engines"),
        host: systemprompt_api::routes::gateway::host_handles(ctx).expect("host handles"),
        db: std::sync::Arc::clone(ctx.db_pool()),
    }
}

async fn harness() -> Harness {
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&boot.database_url).await;
    Harness {
        state: gateway_http_state(&ctx),
    }
}

async fn reject(credential: &str) -> (StatusCode, String) {
    let harness = harness().await;
    let session = SessionId::generate();
    let providers = SubjectProviderSet::default();
    let rejection = authenticate(
        credential,
        AuthInputs {
            header_session: Some(&session),
            headers: &axum::http::HeaderMap::new(),
            providers: &providers,
            state: &harness.state,
        },
    )
    .await
    .expect_err("an unissued credential must never authenticate");
    (rejection.status, rejection.message)
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
