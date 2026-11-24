//! Session binding (credential.md §4): the session a gateway request lands on
//! comes from the credential. An API key with no gateway-issued
//! `x-session-id` gets a session minted and bound to it; a client's own
//! header is recorded as `client_session_id` and never rejected. A JWT always
//! lands on its `session_id` claim, and any other header it sends — the
//! client's own id or another gateway session — is recorded the same way.

use anyhow::Result;
use axum::http::{HeaderMap, StatusCode};
use systemprompt_gateway_entry::http::messages::auth::{AuthInputs, AuthedPrincipal, authenticate};
use systemprompt_gateway_entry::http::messages::error::RejectionError;
use systemprompt_gateway_entry::http::messages::rejection::{
    build_rejection_record, persist_rejection,
};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_identifiers::{AiRequestId, SessionId};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{AppContextSeeds as _, AuthedFixture, seed_admin_credential};
use systemprompt_users::{ApiKeyLimits, ApiKeyService, IssueApiKeyParams, NewApiKey};

use super::common::setup_ctx;
use super::gateway_messages_more::{gw_repos, test_partial};

pub(super) fn jwt_extractor(
    ctx: &AppContext,
) -> systemprompt_gateway_entry::http::state::GatewayHttpState {
    gateway_http_state(ctx)
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

pub(super) async fn sign_in(
    _ctx: &AppContext,
    extractor: &systemprompt_gateway_entry::http::state::GatewayHttpState,
    credential: &str,
    header_session: Option<&SessionId>,
    providers: &SubjectProviderSet,
) -> Result<AuthedPrincipal, RejectionError> {
    authenticate(
        credential,
        AuthInputs {
            header_session,
            headers: &HeaderMap::new(),
            providers,
            state: extractor,
        },
    )
    .await
}

async fn key_for(ctx: &AppContext, cred: &AuthedFixture) -> Result<NewApiKey> {
    Ok(ApiKeyService::new(
        systemprompt_test_fixtures::user_repository(&ctx),
        systemprompt_users::ApiKeyLifetime::unbounded(),
    )
    .issue(IssueApiKeyParams {
        user_id: &cred.user_id,
        name: "session-binding",
        expires_at: None,
        limits: &ApiKeyLimits::default(),
        scopes: &[],
        purpose: None,
        kind: systemprompt_users::ApiKeyKind::User,
    })
    .await?)
}

#[tokio::test]
async fn a_key_without_a_session_header_gets_one_minted_and_bound() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "bind-mint@example.invalid").await?;
    let issued = key_for(&ctx, &cred).await?;
    let extractor = jwt_extractor(&ctx);

    let first = sign_in(
        &ctx,
        &extractor,
        &issued.secret,
        None,
        &SubjectProviderSet::default(),
    )
    .await
    .expect("a key with no header authenticates");
    let second = sign_in(
        &ctx,
        &extractor,
        &issued.secret,
        None,
        &SubjectProviderSet::default(),
    )
    .await
    .expect("and again");

    assert_ne!(first.session_id().as_str(), cred.session_id.as_str());
    assert_eq!(
        first.session_id(),
        second.session_id(),
        "every header-less request on one key lands on its bound session"
    );
    assert!(first.client_session().is_none());
    let key_id = &first.api_key().expect("api key principal").api_key_id;
    let bound = systemprompt_test_fixtures::user_repository(&ctx)
        .find_api_key_session(key_id)
        .await?;
    assert_eq!(bound.as_ref(), Some(first.session_id()));
    Ok(())
}

#[tokio::test]
async fn a_client_session_header_is_recorded_and_never_rejected() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "bind-client@example.invalid").await?;
    let issued = key_for(&ctx, &cred).await?;
    let extractor = jwt_extractor(&ctx);
    let opencode = SessionId::new("ses_01opencodeownsessionid");

    let principal = sign_in(
        &ctx,
        &extractor,
        &issued.secret,
        Some(&opencode),
        &SubjectProviderSet::default(),
    )
    .await
    .expect("a session the gateway never issued is not a reason to refuse");

    assert_ne!(principal.session_id(), &opencode);
    assert_eq!(
        principal.client_session().map(|c| c.as_str()),
        Some(opencode.as_str())
    );

    let bound = sign_in(
        &ctx,
        &extractor,
        &issued.secret,
        None,
        &SubjectProviderSet::default(),
    )
    .await
    .expect("bound session");
    assert_eq!(principal.session_id(), bound.session_id());

    let mut partial = test_partial();
    partial.user_id = Some(principal.user_id().clone());
    partial.session_id = Some(principal.session_id().clone());
    partial.client_session_id = principal.client_session.clone();
    let id = AiRequestId::generate();
    let record = build_rejection_record(None, &id, &partial).expect("record");
    assert_eq!(
        record.client_session_id.as_ref().map(|c| c.as_str()),
        Some(opencode.as_str())
    );
    persist_rejection(
        &gw_repos(ctx.db_pool()),
        &id,
        &partial,
        StatusCode::FORBIDDEN,
        "probe",
    )
    .await;
    let stored: Option<String> =
        sqlx::query_scalar("SELECT client_session_id FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pool.pool().as_ref())
            .await?;
    assert_eq!(stored.as_deref(), Some(opencode.as_str()));
    Ok(())
}

#[tokio::test]
async fn a_jwt_header_naming_another_session_lands_on_the_claim_and_is_recorded() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "bind-jwt@example.invalid").await?;
    let other = seed_admin_credential(&pool, "bind-jwt-other@example.invalid").await?;
    let extractor = jwt_extractor(&ctx);
    let providers = SubjectProviderSet::default();

    for header in [
        SessionId::generate(),
        other.session_id.clone(),
        SessionId::new("sess_forged-and-never-issued"),
    ] {
        let principal = sign_in(
            &ctx,
            &extractor,
            cred.jwt.as_str(),
            Some(&header),
            &providers,
        )
        .await
        .expect("a header that differs from the claim is never a reason to refuse");
        assert_eq!(principal.session_id(), &cred.session_id);
        assert_eq!(
            principal.client_session().map(|c| c.as_str()),
            Some(header.as_str())
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_jwt_header_matching_its_claim_or_absent_uses_the_claim() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "bind-jwt-claim@example.invalid").await?;
    let extractor = jwt_extractor(&ctx);
    let providers = SubjectProviderSet::default();

    for header in [None, Some(&cred.session_id)] {
        let principal = sign_in(&ctx, &extractor, cred.jwt.as_str(), header, &providers)
            .await
            .expect("the claim's own session binds");
        assert_eq!(principal.session_id(), &cred.session_id);
        assert!(principal.client_session().is_none());
    }
    Ok(())
}

#[tokio::test]
async fn a_jwt_clients_own_session_header_is_recorded_and_never_rejected() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "bind-jwt-client@example.invalid").await?;
    let extractor = jwt_extractor(&ctx);
    let opencode = SessionId::new("ses_e8d6opencodeownsession");

    let principal = sign_in(
        &ctx,
        &extractor,
        cred.jwt.as_str(),
        Some(&opencode),
        &SubjectProviderSet::default(),
    )
    .await
    .expect("a session the gateway never issued is not a reason to refuse");
    assert_eq!(principal.session_id(), &cred.session_id);
    assert_eq!(
        principal.client_session().map(|c| c.as_str()),
        Some(opencode.as_str())
    );
    Ok(())
}
