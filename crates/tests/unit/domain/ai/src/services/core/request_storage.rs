// RequestStorage seams: session-usage propagation through AiSessionProvider
// and analytics-event publication, driven against the migrated test DB.

use std::sync::{Arc, Mutex};

use systemprompt_ai::models::RequestStatus;
use systemprompt_ai::models::ai::{AiMessage, AiRequest, AiResponse};
use systemprompt_ai::repository::AiRequestRepository;
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
    RequestStorage::new(AiRequestRepository::new(pool).expect("repo"), provider)
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
    let ctx = ctx.with_actor(systemprompt_identifiers::Actor::system(system_user));
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
