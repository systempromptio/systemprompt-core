//! DB-backed tests for `LeaseRenewer`: a lost lease cancels the run token.
//! Tests fail when `DATABASE_URL` is unset.

use std::time::Duration;

use systemprompt_headless::lease::{LeaseRenewer, LeaseSpec};
use tokio_util::sync::CancellationToken;

use crate::repository::{fixture, queue};

#[tokio::test]
async fn renewer_keeps_a_held_lease_and_cancels_on_loss() {
    let f = fixture().await;
    let id = queue(&f).await;
    let lease = Duration::from_secs(90);
    f.repo
        .claim_queued(&f.origin_kind, "a", lease)
        .await
        .expect("claim")
        .expect("row");

    let token = CancellationToken::new();
    let spec = LeaseSpec {
        run_id: id.clone(),
        claimed_by: "a".into(),
        every: Duration::from_millis(50),
        lease,
    };
    let renewer = LeaseRenewer::start(f.repo.clone(), spec, token.clone());
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!token.is_cancelled());

    sqlx::query("UPDATE headless_runs SET claimed_by = 'b' WHERE id = $1")
        .bind(id.as_str())
        .execute(f.repo.write_pool())
        .await
        .expect("steal");
    tokio::time::timeout(Duration::from_secs(5), token.cancelled())
        .await
        .expect("token cancelled after lease loss");
    renewer.stop().await;
}
