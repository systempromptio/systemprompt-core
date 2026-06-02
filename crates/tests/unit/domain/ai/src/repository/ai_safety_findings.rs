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
        })
        .await
        .expect("insert");
    assert!(!id.as_str().is_empty());
}
