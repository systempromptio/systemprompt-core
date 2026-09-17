//! DB-backed tests for who may join a client-reported result to an execution
//! the platform observed: only the user that execution belongs to.

use rmcp::model::{CallToolResult, ContentBlock, MetaObject};
use serde_json::json;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ContextId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::{ArtifactIngest, IngestRequest};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::EXECUTION_META_KEY;
use systemprompt_models::auth::UserType;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

const OWNER: &str = "11111111-1111-4111-8111-111111111abc";
const STRANGER: &str = "22222222-2222-4222-8222-222222222abc";

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

fn ctx(user: Option<&str>) -> RequestContext {
    let session = unique("sess");
    let ctx = RequestContext::new(
        SessionId::new(session.clone()),
        TraceId::new(session),
        ContextId::generate(),
        AgentName::try_new("ownership-tests").expect("valid AgentName"),
    );
    match user {
        Some(user) => ctx
            .with_actor(Actor::user(UserId::new(user)))
            .with_user_type(UserType::User),
        None => ctx,
    }
}

fn request(
    result: CallToolResult,
    ctx: RequestContext,
    call: Option<&AiToolCallId>,
    source: ExecutionSource,
) -> IngestRequest {
    IngestRequest {
        result,
        tool_name: "Read".to_owned(),
        server_name: Some("tests".to_owned()),
        ai_tool_call_id: call.cloned(),
        mcp_execution_id: None,
        ctx,
        skill: None,
        source,
        started_at: None,
        input: None,
    }
}

fn with_meta(text: &str, execution_id: &str) -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text(text)]);
    let mut meta = serde_json::Map::new();
    meta.insert(
        EXECUTION_META_KEY.to_owned(),
        json!({"mcp_execution_id": execution_id}),
    );
    result.meta = Some(MetaObject(meta));
    result
}

#[tokio::test]
async fn another_users_tool_call_id_does_not_join_their_execution() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, None).expect("ingest");
    let call = AiToolCallId::new(unique("toolu"));

    let owned = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text("owner body")]),
            ctx(Some(OWNER)),
            Some(&call),
            ExecutionSource::Proxy,
        ))
        .await
        .expect("owner ingest");

    let foreign = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text("stranger body")]),
            ctx(Some(STRANGER)),
            Some(&call),
            ExecutionSource::HookClaudeCode,
        ))
        .await
        .expect("stranger ingest");
    assert!(
        foreign.created,
        "the stranger gets an execution of their own"
    );
    assert_ne!(foreign.mcp_execution_id, owned.mcp_execution_id);
    assert_ne!(foreign.artifact_id, owned.artifact_id);
    let stranger_artifact = ingest
        .artifacts()
        .find_by_id(&foreign.artifact_id)
        .await
        .unwrap()
        .expect("stranger artifact stored");
    assert_eq!(
        stranger_artifact.ai_tool_call_id, None,
        "the foreign key is not recorded on the stranger's rows"
    );
    let owner_artifact = ingest
        .artifacts()
        .find_by_id(&owned.artifact_id)
        .await
        .unwrap()
        .expect("owner artifact stored");
    assert_eq!(owner_artifact.ai_tool_call_id, Some(call.clone()));

    let same_user = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text("owner body")]),
            ctx(Some(OWNER)),
            Some(&call),
            ExecutionSource::HookClaudeCode,
        ))
        .await
        .expect("owner hook ingest");
    assert!(!same_user.created, "the owner's own hook still joins");
    assert_eq!(same_user.mcp_execution_id, owned.mcp_execution_id);
}

#[tokio::test]
async fn another_users_execution_id_in_meta_does_not_join_and_anonymous_never_does() {
    let Some(db) = db_or_skip().await else { return };
    let ingest = ArtifactIngest::from_db(&db, None).expect("ingest");

    let owned = ingest
        .ingest(request(
            CallToolResult::success(vec![ContentBlock::text("owner body")]),
            ctx(Some(OWNER)),
            Some(&AiToolCallId::new(unique("toolu"))),
            ExecutionSource::Proxy,
        ))
        .await
        .expect("owner ingest");

    let foreign = ingest
        .ingest(request(
            with_meta("stranger body", owned.mcp_execution_id.as_str()),
            ctx(Some(STRANGER)),
            None,
            ExecutionSource::Gateway,
        ))
        .await
        .expect("stranger ingest");
    assert_ne!(foreign.mcp_execution_id, owned.mcp_execution_id);

    let anonymous = ingest
        .ingest(request(
            with_meta("anonymous body", owned.mcp_execution_id.as_str()),
            ctx(None),
            None,
            ExecutionSource::HookOpenCode,
        ))
        .await
        .expect("anonymous ingest");
    assert_ne!(anonymous.mcp_execution_id, owned.mcp_execution_id);

    let owner_again = ingest
        .ingest(request(
            with_meta("owner body", owned.mcp_execution_id.as_str()),
            ctx(Some(OWNER)),
            None,
            ExecutionSource::HookOpenCode,
        ))
        .await
        .expect("owner ingest again");
    assert_eq!(owner_again.mcp_execution_id, owned.mcp_execution_id);
}
