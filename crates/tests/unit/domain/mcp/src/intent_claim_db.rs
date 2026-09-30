//! DB-backed tests for claiming a model's tool-call intent from the
//! execution side: one intent goes to exactly one execution however many
//! same-tool calls race for it, a tool name never matches by LIKE
//! wildcard, and an intent whose call id an execution already carries is
//! reconciled to that execution rather than claimed a second time.

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ContextId, McpExecutionId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::IntentClaimService;
use systemprompt_mcp::models::ToolExecutionRequest;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::{Correlation, ExecutionSource};
use systemprompt_test_fixtures::{seed_user_row, seed_user_session, test_db_pool};

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

fn claims(
    db: &systemprompt_database::DbPool,
    repo: &Arc<ToolUsageRepository>,
) -> IntentClaimService {
    IntentClaimService::new(
        Arc::new(AiRequestRepository::new(db).expect("ai repository")),
        Arc::clone(repo),
    )
}

async fn started_execution(repo: &ToolUsageRepository, session: &str) -> McpExecutionId {
    started_execution_for(repo, session, None).await
}

async fn started_execution_for(
    repo: &ToolUsageRepository,
    session: &str,
    ai_tool_call_id: Option<AiToolCallId>,
) -> McpExecutionId {
    let ctx = RequestContext::new(
        SessionId::new(session),
        TraceId::new(session),
        ContextId::generate(),
        AgentName::try_new("intent-tests").expect("valid AgentName"),
    )
    .with_actor(Actor::user(UserId::new("intent-user")));
    let request = ToolExecutionRequest {
        tool_name: "Read".to_owned(),
        server_name: "intent-tests".to_owned(),
        input: json!({}),
        started_at: Utc::now(),
        context: ctx,
        request_method: Some("mcp".to_owned()),
        request_source: Some("intent-tests".to_owned()),
        ai_tool_call_id,
        source: ExecutionSource::InProcess,
    };
    let exec_id = McpExecutionId::generate();
    repo.start_execution(&exec_id, &request, Correlation::Exact)
        .await
        .expect("start execution");
    exec_id
}

async fn seed_intents(
    db: &systemprompt_database::DbPool,
    session: &str,
    tools: &[&str],
) -> Vec<String> {
    let raw = db.pool_arc().expect("raw pool");
    let user = UserId::new("11111111-1111-4111-8111-111111111abd");
    seed_user_row(db, &user, "intent-user@tests.invalid")
        .await
        .expect("seed user");
    seed_user_session(db, &user, &SessionId::new(session))
        .await
        .expect("seed session");
    let request_id = unique("req");
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, session_id, context_id, provider, model, actor_kind, actor_id) \
         VALUES ($1, $1, $4, $2, $3, 'test', 'test-model', 'user', $4)",
    )
    .bind(&request_id)
    .bind(session)
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(user.as_str())
    .execute(raw.as_ref())
    .await
    .expect("seed ai_request");
    let mut calls = Vec::new();
    for (sequence, tool) in tools.iter().enumerate() {
        let call_id = unique("toolu");
        sqlx::query(
            "INSERT INTO ai_request_tool_calls (request_id, tool_name, tool_input, ai_tool_call_id, sequence_number) \
             VALUES ($1, $2, '{}', $3, $4)",
        )
        .bind(&request_id)
        .bind(tool)
        .bind(&call_id)
        .bind(i32::try_from(sequence).expect("small"))
        .execute(raw.as_ref())
        .await
        .expect("seed tool call");
        calls.push(call_id);
    }
    calls
}

#[tokio::test]
async fn two_executions_of_one_tool_claim_two_different_intents_and_a_third_gets_none() {
    let db = test_db_pool().await;
    let repo = Arc::new(ToolUsageRepository::new(&db).unwrap());
    let claims = claims(&db, &repo);
    let session = unique("sess");
    let seeded = seed_intents(&db, &session, &["Read", "Read"]).await;
    let session_id = SessionId::new(session);

    let first_exec = started_execution(&repo, session_id.as_str()).await;
    let second_exec = started_execution(&repo, session_id.as_str()).await;
    let third_exec = started_execution(&repo, session_id.as_str()).await;
    let (first, second) = tokio::join!(
        claims.claim_inferred(&session_id, "Read", &first_exec, 120),
        claims.claim_inferred(&session_id, "Read", &second_exec, 120),
    );
    let first = first.unwrap().expect("first claim");
    let second = second.unwrap().expect("second claim");
    assert_ne!(first, second, "concurrent claims never share an intent");
    assert!(seeded.contains(&first.to_string()));
    assert!(seeded.contains(&second.to_string()));

    let third = claims
        .claim_inferred(&session_id, "Read", &third_exec, 120)
        .await
        .unwrap();
    assert!(third.is_none(), "every intent is claimed exactly once");

    let first_row = repo.find_by_id(&first_exec).await.unwrap().expect("row");
    assert_eq!(first_row.ai_tool_call_id, Some(first.clone()));
    assert_eq!(first_row.correlation, Correlation::Inferred);
    let third_row = repo.find_by_id(&third_exec).await.unwrap().expect("row");
    assert_eq!(third_row.ai_tool_call_id, None);
    assert_eq!(third_row.correlation, Correlation::Exact);

    let raw = db.pool_arc().expect("raw pool");
    let stamped: Vec<String> = sqlx::query_scalar(
        "SELECT mcp_execution_id FROM ai_request_tool_calls WHERE ai_tool_call_id = ANY($1) ORDER BY mcp_execution_id",
    )
    .bind(&seeded)
    .fetch_all(raw.as_ref())
    .await
    .unwrap();
    let mut expected = vec![first_exec.to_string(), second_exec.to_string()];
    expected.sort();
    assert_eq!(
        stamped, expected,
        "each intent carries the execution that claimed it"
    );
}

#[tokio::test]
async fn a_tool_name_with_an_underscore_does_not_match_another_tool_by_wildcard() {
    let db = test_db_pool().await;
    let repo = Arc::new(ToolUsageRepository::new(&db).unwrap());
    let claims = claims(&db, &repo);
    let session = unique("sess");
    seed_intents(&db, &session, &["axb", "mcp__srv__a_b"]).await;
    let session_id = SessionId::new(session);

    let exec = started_execution(&repo, session_id.as_str()).await;
    let claimed = claims
        .claim_inferred(&session_id, "a_b", &exec, 120)
        .await
        .unwrap()
        .expect("the prefixed exact tool name is claimed");
    let raw = db.pool_arc().expect("raw pool");
    let tool: String = sqlx::query_scalar(
        "SELECT tool_name FROM ai_request_tool_calls WHERE ai_tool_call_id = $1",
    )
    .bind(claimed.as_str())
    .fetch_one(raw.as_ref())
    .await
    .unwrap();
    assert_eq!(tool, "mcp__srv__a_b");

    let other = started_execution(&repo, session_id.as_str()).await;
    let none = claims
        .claim_inferred(&session_id, "a_b", &other, 120)
        .await
        .unwrap();
    assert!(none.is_none(), "`axb` is not `a_b`");
}

#[tokio::test]
async fn an_explicit_intent_claim_is_first_writer_wins() {
    let db = test_db_pool().await;
    let repo = Arc::new(ToolUsageRepository::new(&db).expect("repository"));
    let claims = claims(&db, &repo);
    let session = unique("explicit-claim");
    let seeded = seed_intents(&db, &session, &["Read", "Write"]).await;
    let first = AiToolCallId::new(&seeded[0]);
    let second = AiToolCallId::new(&seeded[1]);
    let exec_one = started_execution(&repo, &session).await;
    let exec_two = started_execution(&repo, &session).await;

    assert!(
        claims
            .claim_exact(&first, &exec_one)
            .await
            .expect("first claim")
    );
    assert!(
        !claims
            .claim_exact(&first, &exec_two)
            .await
            .expect("competing claim is a no-op")
    );

    let raw = db.pool_arc().expect("raw pool");
    let owner: Option<String> = sqlx::query_scalar(
        "SELECT mcp_execution_id FROM ai_request_tool_calls WHERE ai_tool_call_id = $1",
    )
    .bind(first.as_str())
    .fetch_one(raw.as_ref())
    .await
    .expect("claimed intent");
    assert_eq!(owner.as_deref(), Some(exec_one.as_str()));
    let unrelated: Option<String> = sqlx::query_scalar(
        "SELECT mcp_execution_id FROM ai_request_tool_calls WHERE ai_tool_call_id = $1",
    )
    .bind(second.as_str())
    .fetch_one(raw.as_ref())
    .await
    .expect("unrelated intent");
    assert_eq!(unrelated, None);
}

#[tokio::test]
async fn an_intent_an_execution_already_carries_is_handed_to_it_and_the_next_is_claimed() {
    let db = test_db_pool().await;
    let repo = Arc::new(ToolUsageRepository::new(&db).expect("repository"));
    let claims = claims(&db, &repo);
    let session = unique("reconcile");
    let seeded = seed_intents(&db, &session, &["Read", "Read"]).await;
    let older = AiToolCallId::new(&seeded[0]);
    let newer = AiToolCallId::new(&seeded[1]);
    let raw = db.pool_arc().expect("raw pool");
    sqlx::query(
        "UPDATE ai_request_tool_calls SET created_at = NOW() - interval '5 seconds' \
         WHERE ai_tool_call_id = $1",
    )
    .bind(older.as_str())
    .execute(raw.as_ref())
    .await
    .expect("age the older intent");

    let holder = started_execution_for(&repo, &session, Some(newer.clone())).await;
    let inferred = started_execution(&repo, &session).await;
    let claimed = claims
        .claim_inferred(&SessionId::new(&session), "Read", &inferred, 120)
        .await
        .expect("claim")
        .expect("the older intent is still free");
    assert_eq!(claimed, older);

    let owner = |call: AiToolCallId| {
        let raw = Arc::clone(&raw);
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT mcp_execution_id FROM ai_request_tool_calls WHERE ai_tool_call_id = $1",
            )
            .bind(call.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("intent row")
        }
    };
    assert_eq!(owner(newer).await.as_deref(), Some(holder.as_str()));
    assert_eq!(
        owner(older.clone()).await.as_deref(),
        Some(inferred.as_str())
    );
    let row = repo.find_by_id(&inferred).await.unwrap().expect("row");
    assert_eq!(row.ai_tool_call_id, Some(older));
    assert_eq!(row.correlation, Correlation::Inferred);
}
