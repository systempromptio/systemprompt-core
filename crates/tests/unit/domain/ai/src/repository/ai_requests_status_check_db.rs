//! The `ai_requests.status` CHECK constraint: the column admits only the
//! `RequestStatus` vocabulary, so a row can never carry a status no reader
//! parses.

use systemprompt_traits::RequestStatus;

use super::{bootstrapped_pool, seed_request, user};

#[tokio::test]
async fn a_status_outside_the_vocabulary_is_refused_by_the_database() {
    let pool = bootstrapped_pool().await;
    let owner = user();
    let request_id = seed_request(&pool, &owner).await;

    let err = sqlx::query("UPDATE ai_requests SET status = 'bogus' WHERE id = $1")
        .bind(request_id.as_str())
        .execute(pool.write_pool().as_ref())
        .await
        .expect_err("a status outside the CHECK set must be refused");
    let db_err = err.as_database_error().expect("database error");
    assert_eq!(
        db_err.constraint(),
        Some("ai_requests_status_check"),
        "{db_err}"
    );

    for status in [
        RequestStatus::Completed,
        RequestStatus::Failed,
        RequestStatus::Rejected,
    ] {
        sqlx::query("UPDATE ai_requests SET status = $2 WHERE id = $1")
            .bind(request_id.as_str())
            .bind(status.as_str())
            .execute(pool.write_pool().as_ref())
            .await
            .expect("every RequestStatus value is admitted");
    }
}
