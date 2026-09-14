use super::*;

#[tokio::test]
async fn concurrent_workers_claim_disjoint_rows_and_stale_worker_cannot_complete() {
    let f = Fixture::new().await;
    for id in ["a", "b"] {
        f.repository
            .submit(&f.owner, &invocation(id, 1))
            .await
            .expect("submit");
    }
    let one = TaskId::generate();
    let two = TaskId::generate();
    let (first, second) = tokio::join!(
        f.repository.claim(&f.owner, &one, 1, 1),
        f.repository.claim(&f.owner, &two, 1, 1)
    );
    let first = first.expect("first");
    let second = second.expect("second");
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].change_id, second[0].change_id);
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let recovered = f
        .repository
        .claim(&f.owner, &TaskId::generate(), 2, 60)
        .await
        .expect("recover expired leases");
    assert_eq!(recovered.len(), 2);
    assert!(f.repository.apply(&f.owner, &first[0]).await.is_err());
    let (a, b) = tokio::join!(
        f.repository.apply(&f.owner, &recovered[0]),
        f.repository.apply(&f.owner, &recovered[1])
    );
    assert!(a.expect("apply a").replaced);
    assert!(b.expect("apply b").replaced);
    assert_eq!(
        f.repository
            .health(&f.owner)
            .await
            .expect("health")
            .generation,
        2
    );
}

#[tokio::test]
async fn transaction_committing_late_remains_discoverable_after_another_checkpoint() {
    let f = Fixture::new().await;
    let late = invocation("late", 1);
    let mut transaction = f.pool.begin().await.expect("transaction");
    FeedbackFactsRepository::submit_in(&mut transaction, &f.owner, &late)
        .await
        .expect("uncommitted change");
    f.repository
        .submit(&f.owner, &invocation("visible", 1))
        .await
        .expect("committed change");
    f.drain().await;
    assert_eq!(
        f.repository
            .health(&f.owner)
            .await
            .expect("health")
            .generation,
        1
    );
    transaction.commit().await.expect("late commit");
    f.drain().await;
    assert!(
        f.repository
            .get_fact(&f.owner, &late.key)
            .await
            .expect("get late")
            .is_some()
    );
    assert_eq!(
        f.repository
            .health(&f.owner)
            .await
            .expect("health")
            .generation,
        2
    );
}

#[tokio::test]
async fn replacement_deltas_and_checkpoint_rollback_are_restart_safe() {
    let f = Fixture::new().await;
    let initial = invocation("i", 1);
    f.repository
        .submit(&f.owner, &initial)
        .await
        .expect("submit");
    f.drain().await;
    let mut corrected = initial.clone();
    corrected.change_id = AnalyticsChangeId::generate();
    corrected.revision = 2;
    if let AnalyticsChangeOperation::Replace {
        fact: NormalizedAnalyticsFact::Invocation(value),
    } = &mut corrected.operation
    {
        value.succeeded = false;
    }
    f.repository
        .submit(&f.owner, &corrected)
        .await
        .expect("correct");
    f.drain().await;
    let lease = f
        .repository
        .claim_deltas(&f.owner, "snapshot-v1", &TaskId::generate(), 64, 1)
        .await
        .expect("claim")
        .expect("batch");
    let deltas = f
        .repository
        .delta_batch(&f.owner, &lease)
        .await
        .expect("deltas");
    assert_eq!(deltas.len(), 2);
    assert!(deltas[0].before.is_none());
    assert!(deltas[1].before.is_some());
    let mut tx = f.pool.begin().await.expect("aggregate transaction");
    FeedbackFactsRepository::complete_delta_batch(&mut tx, &f.owner, &lease)
        .await
        .expect("checkpoint");
    tx.rollback().await.expect("crash rollback");
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let replacement = f
        .repository
        .claim_deltas(&f.owner, "snapshot-v1", &TaskId::generate(), 64, 60)
        .await
        .expect("reclaim")
        .expect("batch");
    let mut tx = f.pool.begin().await.expect("transaction");
    assert!(
        FeedbackFactsRepository::complete_delta_batch(&mut tx, &f.owner, &lease)
            .await
            .is_err()
    );
    tx.rollback().await.expect("rollback stale");
    let mut tx = f.pool.begin().await.expect("transaction");
    FeedbackFactsRepository::complete_delta_batch(&mut tx, &f.owner, &replacement)
        .await
        .expect("complete");
    tx.commit().await.expect("commit");
    assert!(
        f.repository
            .claim_deltas(&f.owner, "snapshot-v1", &TaskId::generate(), 64, 60)
            .await
            .expect("empty")
            .is_none()
    );
}


#[tokio::test]
async fn retry_records_diagnostics_and_fresh_worker_resumes() {
    let f = Fixture::new().await;
    let change = invocation("retry", 1);
    f.repository
        .submit(&f.owner, &change)
        .await
        .expect("submit");
    let lease = f
        .repository
        .claim(&f.owner, &TaskId::generate(), 1, 60)
        .await
        .expect("claim")
        .remove(0);
    f.repository.retry(&f.owner, &lease).await.expect("retry");
    let status = f
        .repository
        .change_status(&f.owner, &change.change_id)
        .await
        .expect("status")
        .expect("change");
    assert_eq!(status.attempts, 1);
    assert!(status.last_error.is_some());
    assert!(f.repository.apply(&f.owner, &lease).await.is_err());
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    f.drain().await;
    assert_eq!(
        f.repository.health(&f.owner).await.expect("health").retries,
        0
    );
    assert!(
        f.repository
            .get_fact(&f.owner, &change.key)
            .await
            .expect("get")
            .is_some()
    );
}
