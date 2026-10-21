//! Request-rate windows on gateway admission: a per-user window refuses the
//! request past its limit with 429 and `Retry-After` even under quota warn
//! mode, an `api_key` window never charges a request made without a key, and
//! a request quota refuses gives its rate-window charge back.

use axum::body::to_bytes;
use systemprompt_api::routes::gateway::messages::dispatch::map_dispatch_error;
use systemprompt_database::DbPool;
use systemprompt_gateway::GatewayRepositories;
use systemprompt_gateway::policies::{GatewayPolicyConfig, GatewayPolicyEntry, GatewayPolicySpec};
use systemprompt_gateway::service::{DispatchError, GatewayService};
use systemprompt_manifest::services::ProviderRegistry;
use systemprompt_models::providers::ApiSurface;
use systemprompt_test_fixtures::{AuthedFixture, DisposableDb, seed_admin_credential};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

struct Fixture {
    database: DisposableDb,
    pool: DbPool,
    cred: AuthedFixture,
    repos: GatewayRepositories,
}

async fn fixture(schema: &str, policy: serde_json::Value) -> anyhow::Result<Fixture> {
    install_provider_api_key();
    let _ = setup_ctx().await?;
    let database = DisposableDb::with_schema(schema).await;
    let pool = database.test_pool().await;
    let cred = seed_admin_credential(&pool, &format!("{schema}@example.invalid")).await?;
    sqlx::query(
        "INSERT INTO ai_gateway_policies (id, name, spec, enabled, priority) \
         VALUES ($1, $1, $2, TRUE, 100)",
    )
    .bind(format!("gwpol_{schema}"))
    .bind(policy)
    .execute(pool.pool().as_ref())
    .await?;
    let repos = gw_repos(&pool);
    Ok(Fixture {
        database,
        pool,
        cred,
        repos,
    })
}

impl Fixture {
    async fn dispatch(
        &self,
        registry: &ProviderRegistry,
    ) -> Result<http::StatusCode, DispatchError> {
        let dispatch = inputs(&self.cred, canonical_request(MODEL, false), false);
        let config = gateway_config(PROVIDER);
        let response =
            GatewayService::dispatch(&config, registry, &self.pool, &self.repos, dispatch).await?;
        let status = response.status();
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("body");
        self.drain().await;
        Ok(status)
    }

    async fn drain(&self) {
        assert_eq!(
            self.repos
                .background
                .drain(std::time::Duration::from_secs(30))
                .await,
            systemprompt_traits::DrainOutcome::Drained
        );
    }

    async fn finish(self) {
        self.drain().await;
        drop(self.repos);
        self.pool.pool().close().await;
        self.database.drop_now().await;
    }
}

async fn upstream(expected: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_rate", "type": "message", "role": "assistant", "model": MODEL,
            "content": [{"type": "text", "text": "ok"}], "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1}
        })))
        .expect(expected)
        .mount(&server)
        .await;
    server
}

fn registry(server: &MockServer) -> ProviderRegistry {
    provider_registry(
        &server.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    )
}

#[tokio::test]
async fn a_user_window_refuses_past_its_limit_with_retry_after() -> anyhow::Result<()> {
    let fx = fixture(
        "rate_win_user",
        serde_json::json!({
            "quota_mode": "warn",
            "quota_windows": [{"window_seconds": 3600, "max_requests": 1}],
            "rate_windows": [{"window_seconds": 3600, "subject": "user", "max_requests": 2}]
        }),
    )
    .await?;
    let server = upstream(2).await;
    let registry = registry(&server);

    assert_eq!(fx.dispatch(&registry).await?, http::StatusCode::OK);
    assert_eq!(fx.dispatch(&registry).await?, http::StatusCode::OK);
    let error = fx
        .dispatch(&registry)
        .await
        .expect_err("the third request exceeds the rate window");
    let response = map_dispatch_error(error).expect("a rate denial renders a response");
    assert_eq!(response.status(), http::StatusCode::TOO_MANY_REQUESTS);
    let retry_after: i64 = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .expect("retry-after header");
    assert!((1..=3600).contains(&retry_after), "{retry_after}");
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await?)?;
    assert_eq!(body["error"]["type"], "rate_limit_error");

    server.verify().await;
    fx.finish().await;
    Ok(())
}

#[tokio::test]
async fn an_api_key_window_does_not_charge_a_keyless_request() -> anyhow::Result<()> {
    let fx = fixture(
        "rate_win_keyless",
        serde_json::json!({
            "rate_windows": [{"window_seconds": 3600, "subject": "api_key", "max_requests": 1}]
        }),
    )
    .await?;
    let server = upstream(3).await;
    let registry = registry(&server);
    for _ in 0..3 {
        assert_eq!(fx.dispatch(&registry).await?, http::StatusCode::OK);
    }
    server.verify().await;
    fx.finish().await;
    Ok(())
}

#[tokio::test]
async fn a_quota_refusal_refunds_the_rate_window_charge() -> anyhow::Result<()> {
    let fx = fixture(
        "rate_win_refund",
        serde_json::json!({
            "quota_mode": "enforce",
            "quota_windows": [{"window_seconds": 3600, "max_requests": 1}],
            "rate_windows": [{"window_seconds": 3600, "subject": "user", "max_requests": 5}]
        }),
    )
    .await?;
    let server = upstream(1).await;
    let registry = registry(&server);

    assert_eq!(fx.dispatch(&registry).await?, http::StatusCode::OK);
    let error = fx
        .dispatch(&registry)
        .await
        .expect_err("the second request exceeds the quota window");
    let response = map_dispatch_error(error).expect("a quota denial renders a response");
    assert_eq!(response.status(), http::StatusCode::TOO_MANY_REQUESTS);

    let (hits, charges): (i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT COALESCE(SUM(hits), 0)::bigint FROM governance_rate_windows \
            WHERE bucket_key LIKE 'gateway_rate_user:%'), \
           (SELECT COUNT(*) FROM governance_rate_charges \
            WHERE bucket_key LIKE 'gateway_rate_user:%')",
    )
    .fetch_one(fx.pool.pool().as_ref())
    .await?;
    assert_eq!(
        (hits, charges),
        (1, 1),
        "only the request that reached the upstream spends request-rate budget"
    );

    server.verify().await;
    fx.finish().await;
    Ok(())
}

#[test]
fn a_zero_rate_window_is_rejected_by_config_validation() {
    let spec: GatewayPolicySpec = serde_json::from_value(serde_json::json!({
        "rate_windows": [{"window_seconds": 0, "subject": "user", "max_requests": 5}]
    }))
    .expect("spec parses");
    let config = GatewayPolicyConfig {
        policies: vec![GatewayPolicyEntry {
            name: "burst".to_owned(),
            enabled: true,
            priority: 0,
            spec,
        }],
    };
    assert!(config.validate().is_err());
}

#[test]
fn an_unknown_rate_subject_is_rejected() {
    let parsed = serde_json::from_value::<GatewayPolicySpec>(serde_json::json!({
        "rate_windows": [{"window_seconds": 10, "subject": "tenant", "max_requests": 5}]
    }));
    assert!(parsed.is_err());
}
