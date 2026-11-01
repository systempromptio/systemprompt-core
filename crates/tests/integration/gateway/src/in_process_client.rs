//! The in-process gateway client drives the full dispatch pipeline as a
//! service principal: the upstream is reached and its answer decoded, the
//! call leaves an audited `ai_requests` row attributed to the job and its
//! purpose, a request guard denies before the upstream, the principal's own
//! quota refuses, and a strict JSON Schema round-trips structured output.

use std::sync::{Arc, Mutex};

use systemprompt_database::DbPool;
use systemprompt_extension::{
    GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard, register_gateway_guard,
};
use systemprompt_gateway::client::{
    GatewayClient, GatewayClientError, GatewayClientParts, ServiceIdentity, ServicePrincipal,
};
use systemprompt_gateway::protocol::canonical::ResponseFormat;
use systemprompt_gateway::{GatewayEngines, GatewayRepositories};
use systemprompt_identifiers::{AiRequestId, ServiceName};
use systemprompt_manifest::services::GatewayConfig;
use systemprompt_test_fixtures::{DisposableDb, seed_admin_credential};
use systemprompt_traits::{DatabaseHandle, SessionProvider};
use systemprompt_users::{SessionRepository, UserRepository};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use self::harness::{MODEL, catalog, repos};
use crate::support::minimal_request;

mod harness;

const PURPOSE: &str = "brain.extract";

static DENIED_USER: Mutex<Option<String>> = Mutex::new(None);

#[derive(Default)]
struct DenyServiceGuard;

#[async_trait::async_trait]
impl GatewayRequestGuard for DenyServiceGuard {
    async fn check(
        &self,
        _db: &dyn DatabaseHandle,
        request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        if DENIED_USER.lock().expect("denied user").as_deref() == Some(request.user_id.as_str()) {
            return Err(GatewayDenyReason::forbidden(
                "service principal is not allowed",
            ));
        }
        Ok(())
    }
}

register_gateway_guard!(DenyServiceGuard);

struct Fixture {
    database: DisposableDb,
    pool: DbPool,
    repos: Arc<GatewayRepositories>,
    client: GatewayClient,
    upstream: MockServer,
}

async fn fixture(schema: &str, policy: Option<serde_json::Value>) -> anyhow::Result<Fixture> {
    // SAFETY: set before the process's first bootstrap; nextest runs each test
    // in its own process.
    unsafe {
        std::env::set_var("ANTHROPIC_API_KEY", "sk-test-anthropic-key");
    }
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let database = DisposableDb::with_schema(schema).await;
    let pool = database.test_pool().await;
    if let Some(spec) = policy {
        sqlx::query(
            "INSERT INTO ai_gateway_policies (id, name, spec, enabled, priority) \
             VALUES ($1, $1, $2, TRUE, 100)",
        )
        .bind(format!("gwpol_{schema}"))
        .bind(spec)
        .execute(pool.pool().as_ref())
        .await?;
    }
    let admin = seed_admin_credential(&pool, &format!("{schema}@example.invalid")).await?;
    let sessions: Arc<dyn SessionProvider> = Arc::new(SessionRepository::new(&pool));
    let principal = ServicePrincipal::resolve(
        &UserRepository::new(&pool),
        sessions,
        ServiceIdentity {
            name: ServiceName::try_new("brain")?,
            email: format!("brain-{schema}@service.invalid"),
            created_by: admin.user_id,
        },
    )
    .await?;
    let upstream = MockServer::start().await;
    let repos = Arc::new(repos(&pool));
    let engines = GatewayEngines::new(
        Default::default(),
        &systemprompt_test_fixtures::offline_extension_context(),
        systemprompt_test_fixtures::ensure_test_secrets_bootstrap(),
    )?;
    let client = GatewayClient::builder(
        GatewayClientParts {
            config: Arc::new(GatewayConfig {
                enabled: true,
                ..GatewayConfig::default()
            }),
            catalog: Arc::new(catalog(&upstream.uri())),
            db: Arc::clone(&pool),
            repos: Arc::clone(&repos),
            engines: Arc::new(engines),
            governance: systemprompt_test_fixtures::default_governance_engine(),
        },
        principal,
    )
    .build();
    Ok(Fixture {
        database,
        pool,
        repos,
        client,
        upstream,
    })
}

impl Fixture {
    async fn mount(&self, body: serde_json::Value, calls: u64) {
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(calls)
            .mount(&self.upstream)
            .await;
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

    async fn audit_row(
        &self,
        id: &AiRequestId,
    ) -> Option<(String, String, String, Option<String>)> {
        sqlx::query_as(
            "SELECT r.actor_kind, r.client_kind, r.status, a.value FROM ai_requests r \
             LEFT JOIN ai_request_attributions a \
               ON a.request_id = r.id AND a.dimension = 'purpose' \
             WHERE r.id = $1 AND r.user_id = $2",
        )
        .bind(id.as_str())
        .bind(self.client.principal().user_id().as_str())
        .fetch_optional(self.pool.pool().as_ref())
        .await
        .expect("query ai_requests")
    }

    async fn finish(self) {
        self.drain().await;
        self.upstream.verify().await;
        drop((self.client, self.repos));
        self.pool.pool().close().await;
        self.database.drop_now().await;
    }
}

fn message(content: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": "msg_in_process", "type": "message", "role": "assistant", "model": MODEL,
        "content": content, "stop_reason": "end_turn",
        "usage": {"input_tokens": 11, "output_tokens": 7}
    })
}

#[tokio::test]
async fn a_completion_reaches_the_upstream_and_leaves_a_job_audit_row() -> anyhow::Result<()> {
    let fx = fixture("inproc_complete", None).await?;
    fx.mount(
        message(serde_json::json!([{"type": "text", "text": "hello brain"}])),
        1,
    )
    .await;
    let completion = fx
        .client
        .complete(minimal_request(Some("be brief"), "hello gateway"), PURPOSE)
        .await?;
    assert_eq!(completion.text, "hello brain");
    assert_eq!(completion.usage.input_tokens, 11);
    assert_eq!(completion.usage.output_tokens, 7);
    fx.drain().await;
    let row = fx.audit_row(&completion.ai_request_id).await;
    let Some((actor_kind, client_kind, status, purpose)) = row else {
        panic!("the in-process call wrote no ai_requests row");
    };
    assert_eq!(actor_kind, "job");
    assert_eq!(client_kind, "internal");
    assert_eq!(status, "completed");
    assert_eq!(purpose.as_deref(), Some(PURPOSE));
    fx.finish().await;
    Ok(())
}

#[tokio::test]
async fn a_denying_guard_refuses_before_the_upstream() -> anyhow::Result<()> {
    let fx = fixture("inproc_guard", None).await?;
    *DENIED_USER.lock().expect("denied user") =
        Some(fx.client.principal().user_id().as_str().to_owned());
    fx.mount(message(serde_json::json!([])), 0).await;
    let error = fx
        .client
        .complete(minimal_request(None, "hello gateway"), PURPOSE)
        .await
        .expect_err("the guard denies the service principal");
    assert!(
        matches!(&error, GatewayClientError::Denied { reason, .. } if reason.message.contains("not allowed")),
        "expected Denied, got {error:?}"
    );
    fx.finish().await;
    Ok(())
}

#[tokio::test]
async fn an_exhausted_principal_quota_refuses_with_quota() -> anyhow::Result<()> {
    let policy = serde_json::json!({
        "quota_windows": [{"window_seconds": 3600, "max_requests": 1}]
    });
    let fx = fixture("inproc_quota", Some(policy)).await?;
    fx.mount(
        message(serde_json::json!([{"type": "text", "text": "ok"}])),
        1,
    )
    .await;
    fx.client
        .complete(minimal_request(None, "first call"), PURPOSE)
        .await?;
    fx.drain().await;
    let error = fx
        .client
        .complete(minimal_request(None, "second call"), PURPOSE)
        .await
        .expect_err("the one-request window is spent");
    assert!(
        matches!(error, GatewayClientError::Quota { .. }),
        "expected Quota, got {error:?}"
    );
    fx.finish().await;
    Ok(())
}

#[tokio::test]
async fn structured_output_round_trips_the_forced_tool_input() -> anyhow::Result<()> {
    let fx = fixture("inproc_structured", None).await?;
    let facts = serde_json::json!({"facts": [{"statement": "the brain extracts"}]});
    fx.mount(
        message(serde_json::json!([{
            "type": "tool_use", "id": "toolu_1", "name": "extraction", "input": facts
        }])),
        1,
    )
    .await;
    let mut request = minimal_request(None, "extract facts");
    request.response_format = Some(ResponseFormat::JsonSchema {
        name: "extraction".to_owned(),
        schema: serde_json::json!({"type": "object"}),
        strict: true,
    });
    let completion = fx.client.complete(request, PURPOSE).await?;
    assert_eq!(completion.structured, Some(facts));
    fx.finish().await;
    Ok(())
}
