//! A denied request is charged in every window it reserved and then revoked
//! from all of them, so it counts in no window whatever order the windows are
//! configured in.

use systemprompt_gateway::quota::{AccountingOutcome, ReserveOutcome, revoke};
use systemprompt_manifest::services::QuotaFaultMode;

use crate::quota_reserve::{admitted, bucket, repos, reserve, window};
use crate::support::{seed_user, setup_db};

async fn deny_then_revoke(tight_first: bool) -> [(i64, i64, i64, i64); 2] {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let repos = repos(&db);
    let tight = window("user", 660, Some(1));
    let loose = window("user", 7260, None);
    let windows = if tight_first {
        [tight, loose]
    } else {
        [loose, tight]
    };

    admitted(reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await);
    let ReserveOutcome::Denied {
        decision,
        reservation,
    } = reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await
    else {
        panic!("the second request exceeds max_requests 1");
    };
    assert!(!decision.allow);
    assert_eq!(decision.window_seconds, 660);
    assert_eq!(
        reservation.windows.len(),
        2,
        "the denial carries every window it was charged in"
    );
    let outcome = revoke(&repos.quota_buckets, &reservation).await;
    assert!(matches!(outcome, AccountingOutcome::Counted), "{outcome:?}");
    [
        bucket(&db, &user, 660).await,
        bucket(&db, &user, 7260).await,
    ]
}

#[tokio::test]
async fn a_revoked_denial_counts_in_no_window_when_the_tight_window_comes_first() {
    assert_eq!(
        deny_then_revoke(true).await,
        [(1, 50, 60, 70), (1, 50, 60, 70)],
        "only the admitted request remains in either window"
    );
}

#[tokio::test]
async fn a_revoked_denial_counts_in_no_window_when_the_tight_window_comes_last() {
    assert_eq!(
        deny_then_revoke(false).await,
        [(1, 50, 60, 70), (1, 50, 60, 70)],
        "only the admitted request remains in either window"
    );
}

#[tokio::test]
async fn a_denial_kept_for_warn_mode_counts_in_every_window() {
    let db = setup_db().await;
    let user = seed_user(&db).await;
    let repos = repos(&db);
    let windows = [window("user", 690, Some(1)), window("user", 7290, None)];

    admitted(reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await);
    let ReserveOutcome::Denied { .. } =
        reserve(&repos, &user, &windows, QuotaFaultMode::Closed).await
    else {
        panic!("the second request exceeds max_requests 1");
    };
    assert_eq!(bucket(&db, &user, 690).await, (2, 100, 120, 140));
    assert_eq!(
        bucket(&db, &user, 7290).await,
        (2, 100, 120, 140),
        "an unrevoked denial is charged in the windows after the denying one too"
    );
}
