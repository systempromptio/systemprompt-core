use systemprompt_scheduler::{LicenceCheckinRepository, LicenceCheckinState};
use systemprompt_test_fixtures::test_db_pool;

async fn repository() -> LicenceCheckinRepository {
    let pool = test_db_pool().await;
    LicenceCheckinRepository::new(pool.write_pool().as_ref().clone())
}

async fn state(repo: &LicenceCheckinRepository) -> LicenceCheckinState {
    repo.find_state()
        .await
        .expect("find state")
        .expect("state row after an attempt")
}

#[tokio::test]
async fn attempts_successes_and_failures_accumulate_on_the_single_row() {
    let repo = repository().await;

    repo.mark_attempt().await.expect("first attempt");
    let before = state(&repo).await;
    assert!(before.last_attempt_at.is_some());
    assert!(before.attempts_total >= 1);

    repo.mark_attempt().await.expect("second attempt");
    let after_attempt = state(&repo).await;
    assert_eq!(after_attempt.attempts_total, before.attempts_total + 1);

    repo.record_failure(Some(503), "upstream unavailable")
        .await
        .expect("record failure");
    let failed = state(&repo).await;
    assert_eq!(failed.last_status, Some(503));
    assert_eq!(failed.last_error.as_deref(), Some("upstream unavailable"));
    assert!(failed.last_error_at.is_some());
    assert_eq!(failed.failures_total, after_attempt.failures_total + 1);

    repo.record_failure(None, "connect refused")
        .await
        .expect("record transport failure");
    let transport = state(&repo).await;
    assert_eq!(transport.last_status, None);
    assert_eq!(transport.last_error.as_deref(), Some("connect refused"));
    assert_eq!(transport.failures_total, failed.failures_total + 1);

    repo.record_success(200).await.expect("record success");
    let ok = state(&repo).await;
    assert_eq!(ok.last_status, Some(200));
    assert_eq!(ok.last_error, None);
    assert_eq!(ok.last_error_at, None);
    assert!(ok.last_success_at.is_some());
    assert_eq!(ok.failures_total, transport.failures_total);
    assert_eq!(ok.attempts_total, after_attempt.attempts_total);
}
