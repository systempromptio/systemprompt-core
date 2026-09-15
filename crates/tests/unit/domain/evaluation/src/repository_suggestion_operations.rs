//! Suggestion retry identity and reservations commit as one bounded operation.
use super::repository_workers::Harness;
use systemprompt_evaluation::models::SuggestionStatus;
use systemprompt_evaluation::repository::experiments::{
    EvaluationLifecycleRepository, SuggestionRequest,
};

async fn input(h: &Harness) -> SuggestionRequest {
    let execution = h.claim().await;
    let detail = h.experiments().get(&h.owner, &h.experiment).await.unwrap();
    SuggestionRequest {
        experiment_id: h.experiment.clone(),
        budget_id: detail.experiment.budget_id,
        operation_key: "reviewed-edit".to_owned(),
        maximum_cost_microdollars: 10,
        supporting_execution_ids: vec![execution.id],
        proposed_changes: serde_json::json!({"files":[{"path":"file.txt","content":"improved"}]}),
        hypothesis: "Fix the retained development failure".to_owned(),
        originating_evidence: serde_json::json!({"source":"retained-development"}),
    }
}
fn repository(h: &Harness) -> EvaluationLifecycleRepository {
    crate::seams::lifecycle(&h.pg, crate::fixture_admission::fixture_admission())
}
#[tokio::test]
async fn concurrent_identical_suggestion_retries_create_one_row_and_one_reservation() {
    let h = Harness::start().await.expect("PostgreSQL");
    let request = input(&h).await;
    let repo = repository(&h);
    let (a, b) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            repo.create_suggestion(&h.owner, &request),
            repo.create_suggestion(&h.owner, &request)
        )
    })
    .await
    .unwrap();
    let id = a.unwrap();
    assert_eq!(id, b.unwrap());
    assert_eq!(h.budget().await, (10, 0));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM eval_suggestions WHERE owner_id=$1")
        .bind(h.owner.as_str())
        .fetch_one(&h.pg)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        repo.suggestion(&h.owner, &id).await.unwrap().status,
        SuggestionStatus::Draft
    );
    let canonical = crate::seams::lifecycle(&h.pg, crate::seams::verified_admission());
    assert_eq!(
        canonical
            .create_suggestion(&h.owner, &request)
            .await
            .unwrap(),
        id,
        "retained replay does not require re-enabling execution"
    );
    let mut conflict = request.clone();
    conflict.hypothesis = "Changed under the same operation identity".to_owned();
    assert!(repo.create_suggestion(&h.owner, &conflict).await.is_err());
    conflict = request.clone();
    conflict.proposed_changes = serde_json::json!({"files":[]});
    assert!(repo.create_suggestion(&h.owner, &conflict).await.is_err());
    assert_eq!(h.budget().await, (10, 0));
    h.cleanup().await;
}
#[tokio::test]
async fn different_budget_and_unsupported_admission_leave_no_suggestion_or_reservation() {
    let h = Harness::start().await.expect("PostgreSQL");
    let mut request = input(&h).await;
    let repo = repository(&h);
    let original = request.budget_id.clone();
    let other = crate::seams::budgets(&h.pg)
        .create_shared(&h.owner, "other-budget", 100)
        .await
        .unwrap();
    request.budget_id = other.clone();
    assert!(repo.create_suggestion(&h.owner, &request).await.is_err());
    assert_eq!(
        crate::seams::budgets(&h.pg)
            .get(&h.owner, &other)
            .await
            .unwrap()
            .reserved,
        0
    );
    request.budget_id = original;
    assert!(
        crate::seams::lifecycle(&h.pg, crate::seams::verified_admission())
            .create_suggestion(&h.owner, &request)
            .await
            .is_err()
    );
    assert_eq!(h.budget().await, (0, 0));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM eval_suggestions WHERE owner_id=$1")
        .bind(h.owner.as_str())
        .fetch_one(&h.pg)
        .await
        .unwrap();
    assert_eq!(count, 0);
    h.cleanup().await;
}
#[tokio::test]
async fn database_insert_rejection_rolls_back_its_new_reservation() {
    let h = Harness::start().await.expect("PostgreSQL");
    let mut request = input(&h).await;
    let repo = repository(&h);
    request.proposed_changes = serde_json::json!({"unrepresentable":"\u{0000}"});
    assert!(repo.create_suggestion(&h.owner, &request).await.is_err());
    assert_eq!(h.budget().await, (0, 0));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM eval_budget_reservations WHERE account_id=$1")
            .bind(request.budget_id.as_str())
            .fetch_one(&h.pg)
            .await
            .unwrap();
    assert_eq!(count, 0);
    request.proposed_changes =
        serde_json::json!({"files":[{"path":"file.txt","content":"recover"}]});
    let id = repo.create_suggestion(&h.owner, &request).await.unwrap();
    assert_eq!(
        repo.suggestion(&h.owner, &id).await.unwrap().hypothesis,
        request.hypothesis
    );
    assert_eq!(h.budget().await, (10, 0));
    h.cleanup().await;
}

#[tokio::test]
async fn historical_suggestions_remain_readable_without_invented_retry_identity() {
    let h = Harness::start().await.expect("PostgreSQL");
    let request = input(&h).await;
    let repo = repository(&h);
    let retained = repo.create_suggestion(&h.owner, &request).await.unwrap();
    sqlx::query("UPDATE eval_suggestions SET operation_key=NULL,operation_digest=NULL WHERE id=$1")
        .bind(retained.as_str())
        .execute(&h.pg)
        .await
        .unwrap();
    assert_eq!(
        repo.suggestion(&h.owner, &retained).await.unwrap().status,
        SuggestionStatus::Draft
    );
    assert!(
        repo.create_suggestion(&h.owner, &request).await.is_err(),
        "unverifiable historical reservation must not become a new retry result"
    );
    assert_eq!(h.budget().await, (10, 0));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM eval_suggestions WHERE owner_id=$1")
        .bind(h.owner.as_str())
        .fetch_one(&h.pg)
        .await
        .unwrap();
    assert_eq!(count, 1);
    h.cleanup().await;
}
