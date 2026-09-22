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
use systemprompt_models::wire::origin::{
    ClientAttestation, ClientEvidence, ClientKind, InboundWireProtocol, RequestOrigin,
};

fn gateway_journal() -> systemprompt_api::services::gateway::audit::journal::GatewayJournal {
    systemprompt_api::services::gateway::audit::journal::GatewayJournal::open(
        systemprompt_test_fixtures::ensure_test_bootstrap()
            .app_paths
            .storage()
            .data(),
        systemprompt_config::SecretsBootstrap::get().expect("secrets bootstrapped"),
    )
    .expect("gateway journal opens")
}


fn gateway_repos(db: &DbPool) -> systemprompt_api::services::gateway::GatewayRepositories {
    systemprompt_api::services::gateway::GatewayRepositories::new(
        db,
        gateway_journal(),
        std::sync::Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(db).expect("context repository"),
        )),
    )
    .expect("gateway repositories")
}

fn dead_pool() -> DbPool {
    let dead = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(250))
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
        origin: RequestOrigin::gateway(
            ClientKind::Other,
            InboundWireProtocol::AnthropicMessages,
            ClientAttestation::None,
        ),
        evidence: ClientEvidence::none(),
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

#[tokio::test]
async fn accounting_failure_recovers_durably_before_and_after_provider_completion() {
    use systemprompt_api::services::gateway::protocol::CanonicalContent;
    use systemprompt_api::services::gateway::protocol::canonical_response::{
        CanonicalResponse, CanonicalUsage,
    };
    for before_completion in [false, true] {
        let db = setup_db().await;
        let owner = seed_user(&db).await;
        let id = AiRequestId::generate();
        let repos = gateway_repos(&db);
        let context = request_ctx(owner.clone(), id.clone());
        let audit = GatewayAudit::new(&repos, context.clone());
        audit
            .open(
                &minimal_request(None, "native fixture"),
                &Bytes::from_static(b"{}"),
            )
            .await
            .expect("open");
        audit
            .pin_pricing(systemprompt_models::services::ModelPricing {
                input_per_million: 1.0,
                output_per_million: 2.0,
                ..Default::default()
            })
            .expect("pin pricing");
        let usage = CanonicalUsage {
            input_tokens: 11,
            output_tokens: 7,
            ..Default::default()
        };
        let response = CanonicalResponse {
            model: "claude-test".to_owned(),
            content: vec![CanonicalContent::text("native fixture response".to_owned())],
            usage,
            ..Default::default()
        };
        if !before_completion {
            audit
                .complete(
                    usage,
                    vec![],
                    &response,
                    &Bytes::from_static(b"{\"text\":\"native fixture response\"}"),
                )
                .await
                .expect("complete");
        }
        let dead = dead_pool();
        let mut unavailable = repos.clone();
        unavailable.requests = std::sync::Arc::new(
            systemprompt_ai::repository::AiRequestRepository::new(&dead)
                .expect("unavailable repository"),
        );
        let faulted = GatewayAudit::new(&unavailable, context);
        assert!(
            faulted
                .accounting_failed("native quota write failed")
                .await
                .is_err(),
            "database failure must leave encrypted fault receipt pending"
        );
        assert_eq!(
            systemprompt_api::services::gateway::audit::journal::recover(&repos.settlement())
                .await
                .expect("recovery"),
            1
        );
        if before_completion {
            audit
                .complete(
                    usage,
                    vec![],
                    &response,
                    &Bytes::from_static(b"{\"text\":\"native fixture response\"}"),
                )
                .await
                .expect("complete after failure marker");
        }
        assert!(
            faulted
                .accounting_failed("native quota write failed")
                .await
                .is_err()
        );
        assert_eq!(
            systemprompt_api::services::gateway::audit::journal::recover(&repos.settlement())
                .await
                .expect("identical recovery"),
            1
        );
        assert_eq!(
            systemprompt_api::services::gateway::audit::journal::recover(&repos.settlement())
                .await
                .expect("empty recovery"),
            0
        );
        let pg = db.pool_arc().expect("pool");
        let row:(String,Option<i32>,Option<i32>,i64,Option<String>)=sqlx::query_as("SELECT status,input_tokens,output_tokens,cost_microdollars,accounting_error FROM ai_requests WHERE id=$1").bind(id.as_str()).fetch_one(pg.as_ref()).await.expect("persisted fault");
        assert_eq!(
            row,
            (
                "failed".to_owned(),
                Some(11),
                Some(7),
                25,
                Some("native quota write failed".to_owned())
            )
        );
        let turns: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM ai_request_messages WHERE request_id=$1 AND role='assistant'",
        )
        .bind(id.as_str())
        .fetch_one(pg.as_ref())
        .await
        .expect("turn count");
        assert_eq!(
            turns, 1,
            "failure receipt recovery cannot duplicate the paid turn"
        );
    }
}
