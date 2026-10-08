//! Per-key limits: a key's model allowlist is a pre-dispatch 403, its
//! request ceiling and budget become an `api_key` quota window that denies
//! with `subject: "api_key"`, and the audit row records the key.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::StatusCode;
use systemprompt_api::routes::gateway::messages::auth::{ApiKeyPrincipal, AuthedPrincipal};
use systemprompt_api::routes::gateway::messages::extract::scope::{
    api_key_windows, enforce_key_model_allowlist,
};
use systemprompt_gateway::service::{DispatchError, GatewayError, GatewayService};
use systemprompt_identifiers::{ScopeDimension, SessionId, TraceId};
use systemprompt_models::attribution::{RequestAttribution, ScopeBinding};
use systemprompt_models::providers::ApiSurface;
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_users::{ApiKeyLimits, ApiKeyService, IssueApiKeyParams, UserApiKey};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

fn principal(record: UserApiKey) -> AuthedPrincipal {
    AuthedPrincipal::ApiKey(ApiKeyPrincipal {
        api_key_id: record.id,
        limits: record.limits,
        scopes: record.scopes,
        user_id: record.user_id,
        trace_id: TraceId::generate(),
        attested_session: SessionId::generate(),
    })
}

async fn issue(
    pool: &systemprompt_database::DbPool,
    user: &systemprompt_identifiers::UserId,
    limits: ApiKeyLimits,
    scopes: &[ScopeBinding],
) -> UserApiKey {
    let service = ApiKeyService::new(Arc::new(systemprompt_users::UserRepository::new(pool)));
    let minted = service
        .issue(IssueApiKeyParams {
            user_id: user,
            name: "limited key",
            expires_at: None,
            limits: &limits,
            scopes,
        })
        .await
        .expect("issue");
    service
        .verify(&minted.secret)
        .await
        .expect("verify")
        .expect("active key")
}

#[tokio::test]
async fn a_model_outside_the_key_allowlist_is_forbidden() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "key-allowlist@example.invalid").await?;
    let limits = ApiKeyLimits {
        model_allowlist: Some(vec![MODEL.to_owned()]),
        ..ApiKeyLimits::default()
    };
    let principal = principal(issue(&pool, &cred.user_id, limits, &[]).await);
    assert!(enforce_key_model_allowlist(&principal, MODEL).is_ok());
    let rejection = enforce_key_model_allowlist(&principal, "claude-other")
        .expect_err("an unlisted model is refused");
    assert_eq!(rejection.status, StatusCode::FORBIDDEN);
    assert_eq!(
        rejection.message,
        "model 'claude-other' is not allowed for this API key"
    );
    Ok(())
}

#[tokio::test]
async fn key_limits_become_an_api_key_window_and_bound_scopes_ride_the_principal()
-> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "key-window@example.invalid").await?;
    let limits = ApiKeyLimits {
        budget_microdollars: Some(9_000),
        max_requests: Some(3),
        request_window_seconds: Some(600),
        ..ApiKeyLimits::default()
    };
    let bound = [ScopeBinding {
        dimension: ScopeDimension::try_new("project")?,
        value: "apollo".to_owned(),
    }];
    let principal = principal(issue(&pool, &cred.user_id, limits, &bound).await);
    let windows = api_key_windows(&principal);
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].subject, "api_key");
    assert_eq!(windows[0].window_seconds, 600);
    assert_eq!(windows[0].max_requests, Some(3));
    assert_eq!(windows[0].max_cost_microdollars, Some(9_000));
    let AuthedPrincipal::ApiKey(key) = &principal else {
        panic!("api key principal");
    };
    assert_eq!(key.scopes, bound.to_vec());

    let unlimited =
        self::principal(issue(&pool, &cred.user_id, ApiKeyLimits::default(), &[]).await);
    assert!(api_key_windows(&unlimited).is_empty());
    Ok(())
}

#[tokio::test]
async fn a_per_key_request_ceiling_denies_the_second_request_as_api_key() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "key-ceiling@example.invalid").await?;
    let limits = ApiKeyLimits {
        max_requests: Some(1),
        request_window_seconds: Some(3600),
        ..ApiKeyLimits::default()
    };
    let principal = principal(issue(&pool, &cred.user_id, limits, &[]).await);
    let AuthedPrincipal::ApiKey(key) = &principal else {
        panic!("api key principal");
    };
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_key", "type": "message", "role": "assistant", "model": MODEL,
            "content": [{"type": "text", "text": "ok"}], "stop_reason": "end_turn",
            "usage": {"input_tokens": 2, "output_tokens": 1}
        })))
        .expect(1)
        .mount(&upstream)
        .await;
    let config = gateway_config(PROVIDER);
    let registry = provider_registry(
        &upstream.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let repos = gw_repos(&pool);
    let keyed = |request| {
        let mut dispatch = inputs(&cred, request, false);
        dispatch.ctx.attribution = RequestAttribution {
            entries: Vec::new(),
            api_key_id: Some(key.api_key_id.clone()),
        };
        dispatch.ctx.api_key_windows = api_key_windows(&principal);
        dispatch
    };

    let first = keyed(canonical_request(MODEL, false));
    let first_id = first.ctx.ai_request_id.clone();
    let response = GatewayService::dispatch(&config, &registry, &pool, &repos, first).await?;
    assert_eq!(response.status(), http::StatusCode::OK);
    to_bytes(response.into_body(), 1024 * 1024).await?;
    assert_eq!(
        repos
            .background
            .drain(std::time::Duration::from_secs(30))
            .await,
        systemprompt_traits::DrainOutcome::Drained
    );

    let denied = GatewayService::dispatch(
        &config,
        &registry,
        &pool,
        &repos,
        keyed(canonical_request(MODEL, false)),
    )
    .await
    .expect_err("the key's ceiling is one request");
    let DispatchError::Recorded(GatewayError::Quota(quota)) = denied else {
        panic!("expected a recorded quota denial, got {denied:?}");
    };
    let detail = quota.detail.expect("window detail");
    assert_eq!(detail.subject, "api_key");
    assert_eq!(detail.limit, Some(1));
    assert_eq!(detail.used, Some(2));

    let (api_key_id,): (Option<String>,) =
        sqlx::query_as("SELECT api_key_id FROM ai_requests WHERE id = $1")
            .bind(first_id.as_str())
            .fetch_one(pool.pool().as_ref())
            .await?;
    assert_eq!(api_key_id.as_deref(), Some(key.api_key_id.as_str()));
    upstream.verify().await;
    Ok(())
}
