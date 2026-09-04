// DB-backed tests for AiSafetyFindingRepository inserts (FK to ai_requests).

use systemprompt_ai::repository::{AiSafetyFindingRepository, InsertSafetyFinding};

use super::{pool, seed_request, user};

#[tokio::test]
async fn insert_returns_generated_id() {
    let Some(pool) = pool().await else {
        return;
    };
    let uid = user();
    let request_id = seed_request(&pool, &uid).await;
    let repo = AiSafetyFindingRepository::new(&pool).expect("repo");

    let id = repo
        .insert(InsertSafetyFinding {
            ai_request_id: &request_id,
            phase: "input",
            severity: "high",
            category: "prompt_injection",
            scanner: "heuristic",
            excerpt: Some("ignore previous instructions"),
            blocked: true,
        })
        .await
        .expect("insert");
    assert!(!id.as_str().is_empty());
}

#[tokio::test]
async fn insert_allows_null_excerpt() {
    let Some(pool) = pool().await else {
        return;
    };
    let uid = user();
    let request_id = seed_request(&pool, &uid).await;
    let repo = AiSafetyFindingRepository::new(&pool).expect("repo");

    let id = repo
        .insert(InsertSafetyFinding {
            ai_request_id: &request_id,
            phase: "output",
            severity: "low",
            category: "pii",
            scanner: "null",
            excerpt: None,
            blocked: false,
        })
        .await
        .expect("insert");
    assert!(!id.as_str().is_empty());
}

#[tokio::test]
async fn the_rollup_separates_findings_from_the_ones_that_blocked() {
    let Some(pool) = pool().await else {
        return;
    };
    let uid = user();
    let request_id = seed_request(&pool, &uid).await;
    let repo = AiSafetyFindingRepository::new(&pool).expect("repo");
    let category = format!("warn_rollup_{}", uuid::Uuid::new_v4().simple());

    for blocked in [true, false, false] {
        repo.insert(InsertSafetyFinding {
            ai_request_id: &request_id,
            phase: "input",
            severity: "medium",
            category: &category,
            scanner: "heuristic",
            excerpt: None,
            blocked,
        })
        .await
        .expect("insert");
    }

    let rows = repo.list_rollup(None, 500).await.expect("rollup");
    let row = rows
        .iter()
        .find(|r| r.category == category)
        .expect("the seeded category must appear in the rollup");
    assert_eq!(row.count, 3);
    assert_eq!(
        row.blocked_count, 1,
        "warn-mode findings must not be counted as blocks"
    );
}
