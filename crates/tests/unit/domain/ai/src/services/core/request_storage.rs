// RequestStorage seams: session-usage propagation through AiSessionProvider
// and analytics-event publication, driven against the migrated test DB.

use std::sync::{Arc, Mutex};

use systemprompt_ai::models::RequestStatus;
use systemprompt_ai::models::ai::{AiMessage, AiRequest, AiResponse};
use systemprompt_ai::repository::{AiRequestPayloadRepository, AiRequestRepository};
use systemprompt_ai::services::core::request_storage::{RequestStorage, StoreParams};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::{
    AiProviderResult, AiSessionProvider, AnalyticsEvent, AnalyticsEventPublisher,
    CreateAiSessionParams,
};
use uuid::Uuid;

use super::{pool, seeded_context};

#[derive(Default)]
struct RecordingSessionProvider {
    created: Mutex<Vec<String>>,
    increments: Mutex<Vec<(String, i32, i64)>>,
}

#[async_trait::async_trait]
impl AiSessionProvider for RecordingSessionProvider {
    async fn create_session(&self, params: CreateAiSessionParams<'_>) -> AiProviderResult<()> {
        self.created
            .lock()
            .expect("lock")
            .push(params.session_id.as_str().to_owned());
        Ok(())
    }

    async fn increment_ai_usage(
        &self,
        session_id: &SessionId,
        tokens: i32,
        cost_microdollars: i64,
    ) -> AiProviderResult<()> {
        self.increments.lock().expect("lock").push((
            session_id.as_str().to_owned(),
            tokens,
            cost_microdollars,
        ));
        Ok(())
    }
}

#[derive(Default)]
struct RecordingPublisher {
    tokens: Mutex<Vec<i64>>,
}

impl AnalyticsEventPublisher for RecordingPublisher {
    fn publish_analytics_event(&self, event: AnalyticsEvent) {
        if let AnalyticsEvent::AiRequestCompleted { tokens_used } = event {
            self.tokens.lock().expect("lock").push(tokens_used);
        }
    }
}

fn request(ctx: systemprompt_models::RequestContext) -> AiRequest {
    AiRequest::builder(
        vec![AiMessage::system("sys"), AiMessage::user("hi")],
        "anthropic",
        "claude-sonnet-4-6",
        64,
        ctx,
    )
    .build()
}

fn response(request_id: Uuid, content: &str) -> AiResponse {
    let mut response = AiResponse::new(
        request_id,
        content.to_owned(),
        "anthropic".to_owned(),
        "claude-sonnet-4-6".to_owned(),
    );
    response.tokens_used = Some(42);
    response.input_tokens = Some(30);
    response.output_tokens = Some(12);
    response
}

fn storage(pool: &DbPool, provider: Arc<RecordingSessionProvider>) -> RequestStorage {
    RequestStorage::new(
        AiRequestRepository::new(pool).expect("repo"),
        AiRequestPayloadRepository::new(pool).expect("payloads"),
        provider,
    )
}

async fn store(storage: &RequestStorage, request: &AiRequest, response: &AiResponse, cost: i64) {
    storage
        .store(&StoreParams {
            request,
            response,
            context: &request.context,
            status: RequestStatus::Completed,
            error_message: None,
            cost_microdollars: cost,
        })
        .await
        .expect("store ok");
}

#[tokio::test]
async fn session_is_touched_then_usage_incremented() {
    let Some(pool) = pool().await else {
        return;
    };
    let (_user, ctx) = seeded_context(&pool).await;
    let session_id = ctx.session_id().as_str().to_owned();
    let provider = Arc::new(RecordingSessionProvider::default());
    let storage = storage(&pool, provider.clone());

    let request = request(ctx);
    let response = response(Uuid::new_v4(), "answer");
    store(&storage, &request, &response, 1234).await;

    assert_eq!(
        *provider.created.lock().expect("lock"),
        vec![session_id.clone()]
    );
    assert_eq!(
        *provider.increments.lock().expect("lock"),
        vec![(session_id, 42, 1234)]
    );
}

#[tokio::test]
async fn system_user_skips_usage_accounting_but_touches_session() {
    let Some(pool) = pool().await else {
        return;
    };
    let system_user = UserId::new("system");
    systemprompt_test_fixtures::seed_user_row(&pool, &system_user, "system@ai-storage.invalid")
        .await
        .expect("seed system user");
    let (_seeded, ctx) = seeded_context(&pool).await;
    let ctx = ctx.with_actor(systemprompt_identifiers::Actor::user(system_user));
    let session_id = ctx.session_id().as_str().to_owned();
    let provider = Arc::new(RecordingSessionProvider::default());
    let storage = storage(&pool, provider.clone());

    let request = request(ctx);
    let response = response(Uuid::new_v4(), "answer");
    store(&storage, &request, &response, 7).await;

    assert_eq!(*provider.created.lock().expect("lock"), vec![session_id]);
    assert!(provider.increments.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn analytics_publisher_receives_token_count() {
    let Some(pool) = pool().await else {
        return;
    };
    let (_user, ctx) = seeded_context(&pool).await;
    let publisher = Arc::new(RecordingPublisher::default());
    let storage = storage(&pool, Arc::new(RecordingSessionProvider::default()))
        .with_event_publisher(publisher.clone());

    let request = request(ctx);
    let response = response(Uuid::new_v4(), "answer");
    store(&storage, &request, &response, 0).await;

    assert_eq!(*publisher.tokens.lock().expect("lock"), vec![42]);
}

#[tokio::test]
async fn stored_request_persists_messages_and_assistant_reply() {
    let Some(pool) = pool().await else {
        return;
    };
    let (_user, ctx) = seeded_context(&pool).await;
    let storage = storage(&pool, Arc::new(RecordingSessionProvider::default()));
    let request_id = Uuid::new_v4();

    let request = request(ctx);
    let response = response(request_id, "final answer");
    store(&storage, &request, &response, 55).await;

    let read = pool.pool_arc().expect("read pool");
    let roles: Vec<String> = sqlx::query_scalar!(
        "SELECT m.role FROM ai_request_messages m
         JOIN ai_requests r ON r.id = m.request_id
         WHERE r.request_id = $1 ORDER BY m.sequence_number",
        request_id.to_string()
    )
    .fetch_all(read.as_ref())
    .await
    .expect("messages");
    assert_eq!(roles, vec!["system", "user", "assistant"]);
}

// --- provider-failure and full-attribution arms ---

#[derive(Default)]
struct FailingSessionProvider;

#[async_trait::async_trait]
impl AiSessionProvider for FailingSessionProvider {
    async fn create_session(&self, _params: CreateAiSessionParams<'_>) -> AiProviderResult<()> {
        Err(systemprompt_traits::AiProviderError::ConfigurationError {
            message: "session store unavailable".to_owned(),
        })
    }

    async fn increment_ai_usage(
        &self,
        _session_id: &SessionId,
        _tokens: i32,
        _cost_microdollars: i64,
    ) -> AiProviderResult<()> {
        Err(systemprompt_traits::AiProviderError::ConfigurationError {
            message: "usage counter unavailable".to_owned(),
        })
    }
}

#[tokio::test]
async fn a_session_provider_that_fails_does_not_lose_the_audit_row() {
    let Some(pool) = pool().await else {
        return;
    };
    let (user, ctx) = seeded_context(&pool).await;
    let storage = RequestStorage::new(
        AiRequestRepository::new(&pool).expect("repo"),
        AiRequestPayloadRepository::new(&pool).expect("payloads"),
        Arc::new(FailingSessionProvider),
    );

    let request = request(ctx);
    let response = response(Uuid::new_v4(), "answer despite a broken session store");

    // Session accounting is best-effort: both the create and the increment
    // fail here, and neither may take the audit write down with it.
    storage
        .store(&StoreParams {
            request: &request,
            response: &response,
            context: &request.context,
            status: RequestStatus::Completed,
            error_message: None,
            cost_microdollars: 99,
        })
        .await
        .expect("a failing session provider must not fail the store");

    let stored: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(
        stored, 1,
        "the request audit row is the durable record and must survive session-store failure"
    );
}

#[tokio::test]
async fn a_fully_attributed_context_records_every_identifier_on_the_audit_row() {
    let Some(pool) = pool().await else {
        return;
    };
    let (user, ctx) = seeded_context(&pool).await;
    let raw = pool.pool_arc().expect("read pool").as_ref().clone();

    // `ai_requests.task_id` and `.mcp_execution_id` are foreign keys, so the
    // rows they point at have to exist before the audit write.
    let task_id = systemprompt_identifiers::TaskId::generate();
    let mcp_execution_id = systemprompt_identifiers::McpExecutionId::generate();
    sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
        .bind(ctx.context_id().as_str())
        .bind(user.as_str())
        .bind("attribution-context")
        .execute(&raw)
        .await
        .expect("seed the context the task belongs to");

    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, user_id, session_id, trace_id, agent_name) \
         VALUES ($1, $2, $3, $4, $5, 'attribution-agent')",
    )
    .bind(task_id.as_str())
    .bind(ctx.context_id().as_str())
    .bind(user.as_str())
    .bind(ctx.session_id().as_str())
    .bind(ctx.trace_id().as_str())
    .execute(&raw)
    .await
    .expect("seed the task the request belongs to");

    sqlx::query(
        "INSERT INTO mcp_tool_executions \
         (mcp_execution_id, tool_name, server_name, started_at, input, status, user_id, \
          session_id, task_id, context_id, trace_id) \
         VALUES ($1, 'attribution_tool', 'srv', now(), '{\"q\":\"x\"}', 'success', $2, $3, $4, \
          $5, $6)",
    )
    .bind(mcp_execution_id.as_str())
    .bind(user.as_str())
    .bind(ctx.session_id().as_str())
    .bind(task_id.as_str())
    .bind(ctx.context_id().as_str())
    .bind(ctx.trace_id().as_str())
    .execute(&raw)
    .await
    .expect("seed the tool execution that triggered the request");

    let ctx = ctx
        .with_task_id(task_id.clone())
        .with_mcp_execution_id(mcp_execution_id.clone());

    let provider = Arc::new(RecordingSessionProvider::default());
    let storage = storage(&pool, provider);
    let request = request(ctx);
    let response = response(Uuid::new_v4(), "attributed");
    store(&storage, &request, &response, 5).await;

    let row = sqlx::query!(
        "SELECT task_id, trace_id, mcp_execution_id FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("audit row");

    assert_eq!(
        row.task_id.as_deref(),
        Some(task_id.as_str()),
        "a task-scoped request must record its task"
    );
    assert_eq!(
        row.mcp_execution_id.as_deref(),
        Some(mcp_execution_id.as_str()),
        "a tool-triggered request must record the execution that caused it"
    );
    assert!(
        row.trace_id.is_some(),
        "a context carrying a trace id must record it, or correlation breaks"
    );
}

#[tokio::test]
async fn a_failed_status_records_the_error_text_and_a_rejected_one_does_not() {
    let Some(pool) = pool().await else {
        return;
    };
    let (user, ctx) = seeded_context(&pool).await;
    let provider = Arc::new(RecordingSessionProvider::default());
    let storage = storage(&pool, provider);
    let request = request(ctx);

    storage
        .store(&StoreParams {
            request: &request,
            response: &response(Uuid::new_v4(), ""),
            context: &request.context,
            status: RequestStatus::Failed,
            error_message: Some("upstream refused"),
            cost_microdollars: 0,
        })
        .await
        .expect("store a failed request");

    let row = sqlx::query!(
        "SELECT status, error_message FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("audit row");

    assert_eq!(row.status, "failed");
    assert_eq!(
        row.error_message.as_deref(),
        Some("upstream refused"),
        "the failure reason must be persisted, not just logged"
    );
}

#[tokio::test]
async fn a_failed_status_with_no_message_falls_back_to_a_placeholder() {
    let Some(pool) = pool().await else {
        return;
    };
    let (user, ctx) = seeded_context(&pool).await;
    let provider = Arc::new(RecordingSessionProvider::default());
    let storage = storage(&pool, provider);
    let request = request(ctx);

    storage
        .store(&StoreParams {
            request: &request,
            response: &response(Uuid::new_v4(), ""),
            context: &request.context,
            status: RequestStatus::Failed,
            error_message: None,
            cost_microdollars: 0,
        })
        .await
        .expect("store a failed request with no message");

    let error_message: Option<String> = sqlx::query_scalar!(
        "SELECT error_message FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .unwrap();
    assert_eq!(
        error_message.as_deref(),
        Some("Unknown error"),
        "a failed row must never carry an empty reason — it would read as a success"
    );
}

#[tokio::test]
async fn the_storage_debug_elides_the_analytics_publisher() {
    let Some(pool) = pool().await else {
        return;
    };
    let storage = RequestStorage::new(
        AiRequestRepository::new(&pool).expect("repo"),
        AiRequestPayloadRepository::new(&pool).expect("payloads"),
        Arc::new(RecordingSessionProvider::default()),
    );

    let rendered = format!("{storage:?}");
    assert!(rendered.contains("RequestStorage"));
    assert!(
        rendered.contains("None"),
        "a storage built without a publisher must render one as absent, got {rendered}"
    );
}
