//! `journal::recover`, `pending`, `flush_on_shutdown` and `spawn_recovery`
//! over a journal no other test writes to: unreadable receipts are
//! quarantined, interrupted writes are swept, a receipt whose request is still
//! in flight is left alone, and one whose request ended without a terminal
//! receipt is settled as failed with unknown usage.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bytes::Bytes;
use systemprompt_database::DbPool;
use systemprompt_gateway::audit::journal::{
    GatewayJournal, flush_on_shutdown, pending, recover, spawn_recovery,
};
use systemprompt_gateway::{GatewayAudit, GatewayRepositories, GatewayRequestContext};
use systemprompt_governance::policy::types::AccessScope;
use systemprompt_identifiers::{AiRequestId, ContextId, UserId};
use systemprompt_models::origin::{
    ClientAttestation, ClientEvidence, ClientKind, InboundWireProtocol, RequestOrigin,
};

use crate::support::{minimal_request, seed_user, setup_db};

fn private_state_dir() -> PathBuf {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .app_paths
        .storage()
        .data()
        .join(format!(
            "journal-recovery-{}",
            uuid::Uuid::new_v4().simple()
        ))
}

fn repos_in(db: &DbPool, state_dir: &Path) -> GatewayRepositories {
    GatewayRepositories::new(
        db,
        GatewayJournal::open(
            state_dir,
            systemprompt_config::SecretsBootstrap::encryption_master_key()
                .expect("encryption master key"),
        )
        .expect("gateway journal opens"),
        Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(db),
        )),
        systemprompt_traits::BackgroundTasks::new(),
        systemprompt_traits::CacheEpochs::new(),
    )
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
        hop: None,
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
        attribution: systemprompt_models::attribution::RequestAttribution::none(),
        api_key_windows: Vec::new(),
        access_log: None,
        context_bound: false,
    }
}

async fn admitted_audit(db: &DbPool, repos: &GatewayRepositories) -> (GatewayAudit, AiRequestId) {
    let owner = seed_user(db).await;
    let id = AiRequestId::generate();
    let audit = GatewayAudit::new(repos, request_ctx(owner, id.clone()));
    audit
        .open(
            &minimal_request(None, "journal recovery fixture"),
            &Bytes::from_static(b"{}"),
        )
        .await
        .expect("open");
    audit.commit_admission().await.expect("commit admission");
    (audit, id)
}

fn journal_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("gateway-journal")
}

fn files_with_suffix(dir: &Path, suffix: &str) -> usize {
    std::fs::read_dir(dir)
        .expect("read journal dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(suffix))
        .count()
}

async fn status_of(db: &DbPool, id: &AiRequestId) -> (String, Option<String>) {
    sqlx::query_as("SELECT status, error_message FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .fetch_one(db.pool().as_ref())
        .await
        .expect("ai_requests row")
}

#[tokio::test]
async fn an_empty_journal_recovers_nothing() {
    let db = setup_db().await;
    let state = private_state_dir();
    let repos = repos_in(&db, &state);
    let journal = repos.settlement().journal;
    assert_eq!(pending(&journal).await.expect("count"), 0);
    assert_eq!(recover(&repos.settlement()).await.expect("recover"), 0);
}

#[tokio::test]
async fn unreadable_receipts_are_quarantined_and_temp_files_swept() {
    let db = setup_db().await;
    let state = private_state_dir();
    let repos = repos_in(&db, &state);
    let dir = journal_dir(&state);
    std::fs::write(dir.join("aa.receipt"), b"short").expect("truncated receipt");
    std::fs::write(dir.join("bb.receipt"), vec![7u8; 64]).expect("forged receipt");
    std::fs::write(dir.join("cc.tmp"), b"interrupted").expect("temp file");
    std::fs::write(dir.join("notes.txt"), b"ignored").expect("foreign file");

    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        2
    );
    assert_eq!(recover(&repos.settlement()).await.expect("recover"), 0);

    assert_eq!(files_with_suffix(&dir, ".receipt.bad"), 2);
    assert_eq!(files_with_suffix(&dir, ".tmp"), 0);
    assert!(dir.join("notes.txt").exists());
    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        0
    );
}

#[tokio::test]
async fn an_in_flight_request_is_left_alone_then_settled_failed_once_it_ends() {
    let db = setup_db().await;
    let state = private_state_dir();
    let repos = repos_in(&db, &state);
    let (audit, id) = admitted_audit(&db, &repos).await;

    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        1
    );
    assert_eq!(
        recover(&repos.settlement()).await.expect("recover"),
        0,
        "a receipt whose lease is held belongs to a live request"
    );
    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        1
    );

    drop(audit);
    assert_eq!(recover(&repos.settlement()).await.expect("recover"), 1);
    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        0
    );
    let (status, error) = status_of(&db, &id).await;
    assert_eq!(status, "failed");
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.contains("Accounting incomplete")),
        "{error:?}"
    );
}

#[tokio::test]
async fn the_shutdown_flush_settles_an_abandoned_receipt() {
    let db = setup_db().await;
    let state = private_state_dir();
    let repos = repos_in(&db, &state);
    let (audit, id) = admitted_audit(&db, &repos).await;
    drop(audit);

    flush_on_shutdown(&repos.settlement()).await;
    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        0
    );
    assert_eq!(status_of(&db, &id).await.0, "failed");
}

#[tokio::test]
async fn the_shutdown_flush_reports_but_keeps_a_live_receipt() {
    let db = setup_db().await;
    let state = private_state_dir();
    let repos = repos_in(&db, &state);
    let (audit, _id) = admitted_audit(&db, &repos).await;

    flush_on_shutdown(&repos.settlement()).await;
    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        1,
        "the flush must not settle a request still holding its lease"
    );
    drop(audit);
}

#[tokio::test]
async fn the_recovery_task_settles_by_the_shutdown_flush_at_the_latest() {
    let db = setup_db().await;
    let state = private_state_dir();
    let repos = repos_in(&db, &state);
    let (audit, id) = admitted_audit(&db, &repos).await;
    drop(audit);

    let tasks = systemprompt_traits::BackgroundTasks::new();
    spawn_recovery(repos.settlement(), &tasks);
    assert_eq!(
        tasks.shutdown(std::time::Duration::from_secs(30)).await,
        systemprompt_traits::DrainOutcome::Drained
    );
    assert_eq!(
        pending(&repos.settlement().journal).await.expect("count"),
        0
    );
    assert_eq!(status_of(&db, &id).await.0, "failed");
}
