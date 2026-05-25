use std::sync::Arc;

use bytes::Bytes;
use futures::future::join_all;
use systemprompt_api::services::gateway::{GatewayAudit, GatewayRequestContext};
use systemprompt_identifiers::{AiRequestId, ContextId, GatewayConversationId};

use crate::support::{minimal_request, seed_user, setup_db};

#[tokio::test]
async fn gateway_audit_open_is_atomic_under_concurrent_same_request_id() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let request = minimal_request(Some("shared-system-prompt"), "concurrent first turn");
    let gw_conv = request
        .derived_gateway_conversation_id()
        .expect("gateway conversation id");
    let context_id = ContextId::derived_from_gateway_conversation(&gw_conv);
    let ai_request_id = AiRequestId::generate();
    let body = Bytes::from(r#"{"messages":[{"role":"user","content":"concurrent first turn"}]}"#);

    // Fire N concurrent open() calls with the *same* AiRequestId so that the
    // primary-key contract on `ai_requests` is what protects us against
    // duplicate rows. The contract: at most one ai_requests row, no panics.
    const N: usize = 8;
    let mut handles = Vec::with_capacity(N);
    for _ in 0..N {
        let db_cloned = Arc::clone(&db);
        let ctx = GatewayRequestContext {
            ai_request_id: ai_request_id.clone(),
            user_id: user_id.clone(),
            session_id: None,
            context_id: context_id.clone(),
            gateway_conversation_id: Some(gw_conv.clone()),
            trace_id: None,
            provider: "anthropic".to_string(),
            model: "claude-test".to_string(),
            max_tokens: Some(16),
            is_streaming: false,
            wire_protocol: "anthropic-messages".to_string(),
        };
        let req_clone = request.clone();
        let body_clone = body.clone();
        handles.push(tokio::spawn(async move {
            let audit = GatewayAudit::new(&db_cloned, ctx).expect("audit ctor");
            audit.open(&req_clone, &body_clone).await
        }));
    }
    let results = join_all(handles).await;

    let mut ok_count = 0usize;
    let mut err_count = 0usize;
    for r in results {
        let inner = r.expect("task join");
        if inner.is_ok() {
            ok_count += 1;
        } else {
            err_count += 1;
        }
    }
    assert!(
        ok_count >= 1,
        "at least one concurrent open() must succeed (got {ok_count} ok, {err_count} err)"
    );
    assert_eq!(
        ok_count + err_count,
        N,
        "every task must return either Ok or Err — none may panic or hang"
    );

    let pool = db.pool_arc().expect("read pool");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_requests WHERE id = $1")
        .bind(ai_request_id.as_str())
        .fetch_one(pool.as_ref())
        .await
        .expect("count ai_requests");
    assert_eq!(
        count, 1,
        "primary-key contract: exactly one ai_requests row exists for the contended id"
    );

    // Ensure no orphaned payload rows: at most one payload row keyed by the
    // ai_request_id (the request payload upserted by the winning open()).
    let payload_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_request_payloads WHERE ai_request_id = $1")
            .bind(ai_request_id.as_str())
            .fetch_one(pool.as_ref())
            .await
            .expect("count payloads");
    assert!(
        payload_count <= 1,
        "payload upsert must collapse to one row, found {payload_count}"
    );
}

#[tokio::test]
async fn gateway_audit_open_persists_derived_context_id() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let request = minimal_request(Some("persist-context-id"), "first turn for persistence");
    let gw_conv = request.derived_gateway_conversation_id().unwrap();
    let context_id = ContextId::derived_from_gateway_conversation(&gw_conv);
    let ai_request_id = AiRequestId::generate();

    let ctx = GatewayRequestContext {
        ai_request_id: ai_request_id.clone(),
        user_id,
        session_id: None,
        context_id: context_id.clone(),
        gateway_conversation_id: Some(gw_conv.clone()),
        trace_id: None,
        provider: "anthropic".to_string(),
        model: "claude-test".to_string(),
        max_tokens: Some(16),
        is_streaming: false,
        wire_protocol: "anthropic-messages".to_string(),
    };
    let audit = GatewayAudit::new(&db, ctx).expect("audit ctor");
    audit
        .open(&request, &Bytes::from_static(b"{}"))
        .await
        .expect("open");

    let pool = db.pool_arc().expect("read pool");
    let (stored_context, stored_gateway): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT context_id, gateway_conversation_id FROM ai_requests WHERE id = $1")
            .bind(ai_request_id.as_str())
            .fetch_one(pool.as_ref())
            .await
            .expect("fetch ai_request");
    assert_eq!(stored_context.as_deref(), Some(context_id.as_str()));
    assert_eq!(stored_gateway.as_deref(), Some(gw_conv.as_str()));

    // Sanity: the persisted ContextId is a parseable UUID v5.
    let parsed = uuid::Uuid::parse_str(stored_context.as_deref().unwrap()).expect("uuid");
    assert_eq!(parsed.get_version_num(), 5);
    let _ = GatewayConversationId::try_new(stored_gateway.unwrap()).expect("valid gateway id");
}
