//! DB-backed tests that `DbAuditSink` persists the enforcement surface and
//! the verified client rather than flattening every row to a bare user.

use systemprompt_identifiers::{Actor, AgentId, ClientId, McpServerId, TraceId, UserId};
use systemprompt_security::authz::{
    AuthzAuditSink, AuthzContext, AuthzDecision, AuthzRequest, AuthzSource, DbAuditSink, EntityRef,
    GovernanceDecisionRepository,
};
use systemprompt_test_fixtures::test_db_pool;

fn mcp_request(user: &str, server: &str, chain: Vec<Actor>) -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::McpServer(McpServerId::try_new(server).expect("valid McpServerId")),
        user_id: UserId::new(user),
        actor: None,
        client_id: Some(ClientId::bridge()),
        access_scope: None,
        roles: Vec::new(),
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new(format!("trace-{}", uuid::Uuid::new_v4().simple())),
        session_id: None,
        context: AuthzContext::none(),
        context_id: None,
        task_id: None,
        act_chain: chain,
    }
    .for_actor(Actor::mcp(UserId::new(user), server))
}

async fn find_row(
    pool: &sqlx::PgPool,
    trace_id: &str,
) -> (String, String, String, Option<String>, Option<String>) {
    sqlx::query_as(
        "SELECT actor_kind, actor_id, user_id, agent_id, client_id \
         FROM governance_decisions WHERE trace_id = $1",
    )
    .bind(trace_id)
    .fetch_one(pool)
    .await
    .expect("the sink wrote a row keyed by the request trace")
}

async fn cleanup(pool: &sqlx::PgPool, trace_id: &str) {
    sqlx::query("DELETE FROM governance_decisions WHERE trace_id = $1")
        .bind(trace_id)
        .execute(pool)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn sink_records_the_mcp_surface_and_the_client() {
    let pool = test_db_pool().await.write_pool();
    let sink = DbAuditSink::new(GovernanceDecisionRepository::from_pool(pool.clone()));

    let req = mcp_request(
        "sink-user",
        "comms",
        vec![Actor::user(UserId::new("sink-user"))],
    );
    let trace = req.trace_id.as_str().to_owned();
    sink.record(
        &req,
        &AuthzDecision::Allow,
        AuthzSource::AllowAllUnrestricted,
    )
    .await;

    let (actor_kind, actor_id, user_id, agent_id, client_id) = find_row(&pool, &trace).await;
    assert_eq!(actor_kind, "mcp");
    assert_eq!(actor_id, "comms");
    assert_eq!(user_id, "sink-user");
    assert!(
        agent_id.is_none(),
        "a plain-user delegate chain is not a verified agent"
    );
    assert_eq!(client_id.as_deref(), Some(ClientId::bridge().as_str()));
    cleanup(&pool, &trace).await;
}

#[tokio::test]
async fn sink_records_a_verified_agent_delegate() {
    let pool = test_db_pool().await.write_pool();
    let sink = DbAuditSink::new(GovernanceDecisionRepository::from_pool(pool.clone()));

    let req = mcp_request(
        "sink-user",
        "email",
        vec![Actor::agent(
            UserId::new("sink-user"),
            AgentId::new("planner"),
        )],
    );
    let trace = req.trace_id.as_str().to_owned();
    sink.record(
        &req,
        &AuthzDecision::Allow,
        AuthzSource::AllowAllUnrestricted,
    )
    .await;

    let (_, _, _, agent_id, _) = find_row(&pool, &trace).await;
    assert_eq!(agent_id.as_deref(), Some("planner"));
    cleanup(&pool, &trace).await;
}

async fn context_of(pool: &sqlx::PgPool, trace_id: &str) -> String {
    sqlx::query_scalar("SELECT context_id FROM governance_decisions WHERE trace_id = $1")
        .bind(trace_id)
        .fetch_one(pool)
        .await
        .expect("the sink wrote a row keyed by the request trace")
}

#[tokio::test]
async fn sink_without_a_context_or_session_mints_a_fresh_context_not_the_legacy_row() {
    let pool = test_db_pool().await.write_pool();
    let sink = DbAuditSink::new(GovernanceDecisionRepository::from_pool(pool.clone()));
    let legacy = systemprompt_identifiers::ContextId::legacy_context_row();

    let mut contexts = Vec::new();
    for _ in 0..2 {
        let req = mcp_request("sink-ctx-user", "comms", Vec::new());
        let trace = req.trace_id.as_str().to_owned();
        sink.record(
            &req,
            &AuthzDecision::Allow,
            AuthzSource::AllowAllUnrestricted,
        )
        .await;
        contexts.push(context_of(&pool, &trace).await);
        cleanup(&pool, &trace).await;
    }

    for context in &contexts {
        assert_ne!(
            context,
            legacy.as_str(),
            "no row lands in the legacy context"
        );
        assert!(
            systemprompt_identifiers::ContextId::try_new(context.as_str()).is_ok(),
            "{context}"
        );
    }
    assert_ne!(
        contexts[0], contexts[1],
        "each context-less row gets its own context"
    );
}
