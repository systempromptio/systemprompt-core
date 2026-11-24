//! Admission throughput of the gateway journal: many concurrent requests are
//! admitted and settled as failed, and the run reports requests per second
//! and syncs per request. Group commit shares one sync across every write
//! queued behind a flush, so a request costs at most three.

use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use bytes::Bytes;
use systemprompt_gateway::audit::journal::{GatewayJournal, JournalSync, pending};
use systemprompt_gateway::{GatewayAudit, GatewayRepositories};
use systemprompt_identifiers::{AiRequestId, UserId};

use crate::journal_recovery::{private_state_dir, repos_over, request_ctx};
use crate::support::{minimal_request, seed_user, setup_db};

const REQUESTS: usize = 256;
const CONCURRENCY: usize = 32;

#[derive(Debug, Default)]
struct CountingSync(AtomicUsize);

impl JournalSync for CountingSync {
    fn sync(&self, file: &File) -> std::io::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        file.sync_all()
    }
}

async fn admit_and_fail(repos: GatewayRepositories, owner: UserId) {
    let audit = GatewayAudit::new(&repos, request_ctx(owner, AiRequestId::generate()));
    audit
        .open(
            &minimal_request(None, "journal throughput"),
            &Bytes::from_static(b"{}"),
        )
        .await
        .expect("open");
    audit.commit_admission().await.expect("commit admission");
    audit.fail("throughput fixture").await.expect("fail");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_admissions_settle_and_leave_the_journal_empty() {
    let db = setup_db().await;
    let state = private_state_dir();
    let syncs = Arc::new(CountingSync::default());
    let journal = GatewayJournal::open_with(
        &state,
        systemprompt_test_fixtures::ensure_test_secrets_bootstrap()
            .encryption_master_key()
            .expect("encryption master key"),
        syncs.clone(),
    )
    .expect("gateway journal opens");
    let repos = repos_over(&db, journal);
    let opened = syncs.0.load(Ordering::SeqCst);
    let owner = seed_user(&db).await;
    let started = Instant::now();
    for _ in 0..REQUESTS / CONCURRENCY {
        let mut wave = Vec::new();
        for _ in 0..CONCURRENCY {
            wave.push(tokio::spawn(admit_and_fail(repos.clone(), owner.clone())));
        }
        for handle in wave {
            handle.await.expect("request task");
        }
    }
    let elapsed = started.elapsed();
    let rate = REQUESTS as f64 / elapsed.as_secs_f64();
    let fsyncs = syncs.0.load(Ordering::SeqCst) - opened;
    let per_request = fsyncs as f64 / REQUESTS as f64;
    println!(
        "journal throughput: {REQUESTS} requests in {elapsed:?} = {rate:.0} requests/s, \
         {fsyncs} fsyncs = {per_request:.2} per request"
    );
    assert!(
        fsyncs <= REQUESTS * 3,
        "one admission, one terminal and one removal per request at most"
    );
    assert_eq!(pending(&repos.settlement().journal).expect("count"), 0);
}
