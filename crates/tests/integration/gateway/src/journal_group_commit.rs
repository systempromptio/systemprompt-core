//! The journal acknowledges a write only once its batch is durable: a flush
//! that fails answers every waiter of its batch with the error, nothing it
//! carried is indexed, and the journal goes on writing to a fresh segment. An
//! admission past the configured limit is refused, never dropped.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use bytes::Bytes;
use systemprompt_gateway::audit::journal::{GatewayJournal, JournalSync, pending};
use systemprompt_gateway::error::GatewayAuditError;
use systemprompt_gateway::{GatewayAudit, GatewayRepositories};
use systemprompt_identifiers::{AiRequestId, UserId};
use systemprompt_manifest::profile::QuotaConfig;

use crate::journal_recovery::{private_state_dir, repos_over, request_ctx};
use crate::support::{minimal_request, seed_user, setup_db};

const CALLERS: usize = 8;

#[derive(Debug, Default)]
struct StallThenFail {
    armed: AtomicBool,
    stalled: Mutex<(bool, bool)>,
    turn: Condvar,
    failed_data_syncs: AtomicUsize,
}

impl StallThenFail {
    fn wait_until_stalled(&self) {
        let mut state = self.stalled.lock().expect("stall state");
        while !state.0 {
            state = self.turn.wait(state).expect("stall state");
        }
    }

    fn release(&self) {
        self.stalled.lock().expect("stall state").1 = true;
        self.turn.notify_all();
    }
}

impl JournalSync for StallThenFail {
    fn sync(&self, file: &File) -> std::io::Result<()> {
        if !self.armed.load(Ordering::SeqCst) {
            return file.sync_all();
        }
        if !file.metadata()?.is_dir() {
            self.failed_data_syncs.fetch_add(1, Ordering::SeqCst);
            let mut state = self.stalled.lock().expect("stall state");
            if !state.0 {
                state.0 = true;
                self.turn.notify_all();
                while !state.1 {
                    state = self.turn.wait(state).expect("stall state");
                }
            }
        }
        Err(std::io::Error::other("injected flush failure"))
    }
}

async fn opened_audit(repos: &GatewayRepositories, owner: &UserId) -> GatewayAudit {
    let audit = GatewayAudit::new(repos, request_ctx(owner.clone(), AiRequestId::generate()));
    audit
        .open(
            &minimal_request(None, "group commit fixture"),
            &Bytes::from_static(b"{}"),
        )
        .await
        .expect("open");
    audit
}

fn journal_with(state: &Path, sync: Arc<dyn JournalSync>) -> GatewayJournal {
    GatewayJournal::open_with(
        state,
        &systemprompt_test_fixtures::fixture_master_key(),
        sync,
    )
    .expect("gateway journal opens")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_flush_answers_every_waiter_of_its_batch_with_the_error() {
    let db = setup_db().await;
    let state: PathBuf = private_state_dir();
    let sync = Arc::new(StallThenFail::default());
    let repos = repos_over(&db, journal_with(&state, sync.clone()));
    let owner = seed_user(&db).await;
    let mut audits = Vec::new();
    for _ in 0..CALLERS {
        audits.push(Arc::new(opened_audit(&repos, &owner).await));
    }
    sync.armed.store(true, Ordering::SeqCst);

    let leader = Arc::clone(&audits[0]);
    let first = tokio::spawn(async move { leader.commit_admission().await });
    let stalled = Arc::clone(&sync);
    tokio::task::spawn_blocking(move || stalled.wait_until_stalled())
        .await
        .expect("leader reaches the sync");
    let mut followers = Vec::new();
    for audit in audits.iter().skip(1) {
        let audit = Arc::clone(audit);
        followers.push(tokio::spawn(async move { audit.commit_admission().await }));
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    sync.release();

    let mut outcomes = vec![first.await.expect("leader task")];
    for follower in followers {
        outcomes.push(follower.await.expect("follower task"));
    }
    for outcome in &outcomes {
        assert!(
            matches!(outcome, Err(GatewayAuditError::JournalFlush(_))),
            "every waiter is answered with the flush error: {outcome:?}"
        );
    }
    assert!(
        sync.failed_data_syncs.load(Ordering::SeqCst) < CALLERS,
        "the followers were flushed as one batch"
    );
    assert_eq!(pending(&repos.settlement().journal).expect("count"), 0);

    sync.armed.store(false, Ordering::SeqCst);
    let after = opened_audit(&repos, &owner).await;
    after
        .commit_admission()
        .await
        .expect("the journal writes again on a fresh segment");
    assert_eq!(pending(&repos.settlement().journal).expect("count"), 1);
}

#[tokio::test]
async fn an_admission_past_the_limit_is_refused_and_the_slot_frees_with_its_lease() {
    let db = setup_db().await;
    let state = private_state_dir();
    let journal = journal_with(
        &state,
        Arc::new(systemprompt_gateway::audit::journal::Fsync),
    );
    let repos = repos_over(&db, journal).with_quota_sharding(QuotaConfig {
        admission_limit: std::num::NonZeroU32::MIN,
        ..QuotaConfig::default()
    });
    let owner = seed_user(&db).await;

    let held = opened_audit(&repos, &owner).await;
    held.commit_admission().await.expect("first admission");
    let refused = opened_audit(&repos, &owner).await;
    assert!(matches!(
        refused.commit_admission().await,
        Err(GatewayAuditError::AdmissionLimit { limit: 1 })
    ));

    drop(held);
    let next = opened_audit(&repos, &owner).await;
    next.commit_admission()
        .await
        .expect("a released lease frees its admission");
}
