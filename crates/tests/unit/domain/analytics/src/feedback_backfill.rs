use super::*;

#[tokio::test]
async fn replayed_backfill_pages_resume_atomically_after_repository_restart() {
    let f = Fixture::new().await;
    let job = TaskId::generate();
    f.repository
        .begin_backfill(&f.owner, &job, "fixture")
        .await
        .expect("begin");
    let page = BackfillPage {
        expected_generation: 0,
        next_cursor: "page-1".to_owned(),
        complete: false,
        changes: vec![invocation("a", 1), invocation("b", 1)],
    };
    f.repository
        .append_backfill_page(&f.owner, &job, &page)
        .await
        .expect("page");
    let restarted = f.repository.clone();
    let progress = restarted
        .append_backfill_page(&f.owner, &job, &page)
        .await
        .expect("replay");
    assert_eq!(progress.facts, 2);
    assert_eq!(progress.generation, 1);
    let final_page = BackfillPage {
        expected_generation: 1,
        next_cursor: "end".to_owned(),
        complete: true,
        changes: vec![],
    };
    assert!(
        restarted
            .append_backfill_page(&f.owner, &job, &final_page)
            .await
            .expect("complete")
            .complete
    );
    f.drain().await;
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
async fn invalid_backfill_page_rolls_back_enqueued_changes_and_cursor() {
    let f = Fixture::new().await;
    let job = TaskId::generate();
    f.repository
        .begin_backfill(&f.owner, &job, "fixture")
        .await
        .expect("begin");
    let valid = invocation("valid", 1);
    let invalid = invocation("overflow", u64::MAX);
    let page = BackfillPage {
        expected_generation: 0,
        next_cursor: "bad-page".to_owned(),
        complete: true,
        changes: vec![valid.clone(), invalid],
    };
    assert!(
        f.repository
            .append_backfill_page(&f.owner, &job, &page)
            .await
            .is_err()
    );
    assert!(
        f.repository
            .change_status(&f.owner, &valid.change_id)
            .await
            .expect("status")
            .is_none()
    );
    assert_eq!(
        f.repository
            .backfill(&f.owner, &job)
            .await
            .expect("checkpoint")
            .generation,
        0
    );
}
