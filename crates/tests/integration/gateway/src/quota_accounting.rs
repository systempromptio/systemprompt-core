//! Post-response quota accounting under both `QuotaFaultMode` settings.
//!
//! The response is already served when accounting runs, so a failed write
//! cannot deny the request: under `Open` it is logged and the request stays
//! successful, under `Closed` the audit row is marked failed so the uncounted
//! spend is visible.

use bytes::Bytes;
use systemprompt_api::services::gateway::policy::QuotaWindow;
use systemprompt_api::services::gateway::quota::{
    AccountingOutcome, PostUpdateParams, post_update_tokens,
};
use systemprompt_api::services::gateway::service::finalize::record_accounting_outcome;
use systemprompt_api::services::gateway::{GatewayAudit, GatewayRequestContext};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, ContextId, UserId};
use systemprompt_models::services::QuotaFaultMode;
use systemprompt_security::policy::types::AccessScope;

use crate::support::{minimal_request, seed_user, setup_db};

fn gateway_repos(db: &DbPool) -> systemprompt_api::services::gateway::GatewayRepositories {
    systemprompt_api::services::gateway::GatewayRepositories::new(
        db,
        std::sync::Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(db).expect("context repository"),
        )),
    )
    .expect("gateway repositories")
}

fn dead_pool() -> DbPool {
    let dead = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://nobody:nobody@127.0.0.1:1/does-not-exist")
        .expect("lazy pool");
    std::sync::Arc::new(systemprompt_database::Database::from_pools(
        std::sync::Arc::new(dead.clone()),
        Some(std::sync::Arc::new(dead)),
    ))
}

fn request_ctx(user_id: UserId, ai_request_id: AiRequestId) -> GatewayRequestContext {
    GatewayRequestContext {
        ai_request_id,
        user_id: user_id.clone(),
        session_id: Some(crate::support::session_for(&user_id)),
        context_id: ContextId::generate(),
        gateway_conversation_id: None,
        client_session_id: None,
        trace_id: Some(systemprompt_identifiers::TraceId::generate()),
        access_scope: AccessScope::Unknown,
        client_id: None,
        provider: "anthropic".to_owned(),
        requested_model: Some("claude-requested".to_owned()),
        model: "claude-test".to_owned(),
        max_tokens: Some(16),
        is_streaming: false,
        wire_protocol: "anthropic-messages".to_owned(),
        access_log: None,
    }
}

async fn status_of(db: &DbPool, id: &AiRequestId) -> String {
    let pool = db.pool_arc().expect("read pool");
    sqlx::query_scalar("SELECT status FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .fetch_one(pool.as_ref())
        .await
        .expect("status")
}

async fn record_a_failed_accounting_write(mode: QuotaFaultMode) -> String {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let ai_request_id = AiRequestId::generate();
    let audit = GatewayAudit::new(
        &gateway_repos(&db),
        request_ctx(user_id.clone(), ai_request_id.clone()),
    );
    audit
        .open(
            &minimal_request(Some("accounting"), "one turn"),
            &Bytes::from_static(b"{}"),
        )
        .await
        .expect("open");

    let windows = vec![QuotaWindow {
        window_seconds: 60,
        ..QuotaWindow::default()
    }];
    let outcome = post_update_tokens(
        &dead_pool(),
        &systemprompt_ai::repository::AiQuotaBucketRepository::new(&dead_pool())
            .expect("quota repo"),
        PostUpdateParams {
            user_id: &user_id,
            windows: &windows,
            input_tokens: 10,
            output_tokens: 20,
            cost_microdollars: 5,
        },
    )
    .await;
    assert!(
        matches!(outcome, AccountingOutcome::Faulted { .. }),
        "an unreachable database must not report the spend as counted"
    );
    record_accounting_outcome(&audit, mode, outcome).await;
    status_of(&db, &ai_request_id).await
}

#[tokio::test]
async fn a_failed_accounting_write_leaves_the_request_successful_when_open() {
    let status = record_a_failed_accounting_write(QuotaFaultMode::Open).await;
    assert_ne!(status, "failed", "open mode must not fail the request");
}

#[tokio::test]
async fn a_failed_accounting_write_marks_the_request_failed_when_closed() {
    let status = record_a_failed_accounting_write(QuotaFaultMode::Closed).await;
    assert_eq!(
        status, "failed",
        "closed mode must record the uncounted spend"
    );
}

#[tokio::test]
async fn a_successful_accounting_write_is_counted() {
    let db = setup_db().await;
    let user_id = seed_user(&db).await;
    let windows = vec![QuotaWindow {
        window_seconds: 60,
        ..QuotaWindow::default()
    }];
    let outcome = post_update_tokens(
        &db,
        &systemprompt_ai::repository::AiQuotaBucketRepository::new(&db).expect("quota repo"),
        PostUpdateParams {
            user_id: &user_id,
            windows: &windows,
            input_tokens: 10,
            output_tokens: 20,
            cost_microdollars: 5,
        },
    )
    .await;
    assert!(matches!(outcome, AccountingOutcome::Counted));
}
