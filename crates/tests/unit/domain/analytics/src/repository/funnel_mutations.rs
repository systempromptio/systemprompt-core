//! DB-backed tests for `FunnelRepository` mutations: funnel/step creation,
//! deactivate, delete, and forward-only progress recording + completion.

use systemprompt_analytics::{
    CreateFunnelInput, CreateFunnelStepInput, FunnelMatchType, FunnelRepository,
};
use systemprompt_identifiers::{FunnelId, SessionId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

fn unique_funnel_name() -> String {
    format!("funnel-{}", Uuid::new_v4())
}

fn sample_input(name: &str) -> CreateFunnelInput {
    CreateFunnelInput {
        name: name.to_owned(),
        description: Some("a test funnel".to_owned()),
        steps: vec![
            CreateFunnelStepInput {
                name: "landing".to_owned(),
                match_pattern: "/".to_owned(),
                match_type: FunnelMatchType::UrlExact,
            },
            CreateFunnelStepInput {
                name: "signup".to_owned(),
                match_pattern: "/signup".to_owned(),
                match_type: FunnelMatchType::UrlPrefix,
            },
            CreateFunnelStepInput {
                name: "convert".to_owned(),
                match_pattern: "purchase".to_owned(),
                match_type: FunnelMatchType::EventType,
            },
        ],
    }
}

#[tokio::test]
async fn create_funnel_persists_funnel_and_ordered_steps() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let name = unique_funnel_name();
    let created = repo
        .create_funnel(&sample_input(&name))
        .await
        .expect("create");

    assert_eq!(created.funnel.name, name);
    assert_eq!(created.funnel.description.as_deref(), Some("a test funnel"));
    assert!(created.funnel.is_active);
    assert_eq!(created.steps.len(), 3);
    assert_eq!(created.steps[0].step_order, 0);
    assert_eq!(created.steps[0].name, "landing");
    assert_eq!(created.steps[0].match_type, FunnelMatchType::UrlExact);
    assert_eq!(created.steps[2].step_order, 2);
    assert_eq!(created.steps[2].match_type, FunnelMatchType::EventType);

    // Round-trip through find_by_id confirms the rows actually landed.
    let fetched = repo
        .find_by_id(&created.funnel.id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(fetched.steps.len(), 3);
    assert_eq!(fetched.steps[1].name, "signup");

    repo.delete(&created.funnel.id).await.expect("cleanup");
}

#[tokio::test]
async fn create_funnel_with_no_steps_skips_step_insert() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let name = unique_funnel_name();
    let input = CreateFunnelInput {
        name: name.clone(),
        description: None,
        steps: vec![],
    };
    let created = repo.create_funnel(&input).await.expect("create");
    assert!(created.steps.is_empty());
    assert_eq!(created.funnel.description, None);

    let fetched = repo
        .find_by_id(&created.funnel.id)
        .await
        .expect("find")
        .expect("present");
    assert!(fetched.steps.is_empty());

    repo.delete(&created.funnel.id).await.expect("cleanup");
}

#[tokio::test]
async fn deactivate_sets_is_active_false() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let created = repo
        .create_funnel(&sample_input(&unique_funnel_name()))
        .await
        .expect("create");

    let changed = repo
        .deactivate(&created.funnel.id)
        .await
        .expect("deactivate");
    assert!(changed);

    let fetched = repo
        .find_by_id(&created.funnel.id)
        .await
        .expect("find")
        .expect("present");
    assert!(!fetched.funnel.is_active);

    repo.delete(&created.funnel.id).await.expect("cleanup");
}

#[tokio::test]
async fn deactivate_missing_funnel_returns_false() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let missing = FunnelId::new(format!("missing-{}", Uuid::new_v4()));
    let changed = repo.deactivate(&missing).await.expect("deactivate");
    assert!(!changed);
}

#[tokio::test]
async fn delete_removes_funnel_and_returns_true() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let created = repo
        .create_funnel(&sample_input(&unique_funnel_name()))
        .await
        .expect("create");

    let deleted = repo.delete(&created.funnel.id).await.expect("delete");
    assert!(deleted);

    assert!(
        repo.find_by_id(&created.funnel.id)
            .await
            .expect("find")
            .is_none()
    );

    let deleted_again = repo.delete(&created.funnel.id).await.expect("delete again");
    assert!(!deleted_again);
}

#[tokio::test]
async fn record_progress_inserts_then_advances_forward_only() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let created = repo
        .create_funnel(&sample_input(&unique_funnel_name()))
        .await
        .expect("create");
    let funnel_id = created.funnel.id.clone();
    let session_id = SessionId::new(format!("sess-{}", Uuid::new_v4()));

    // First record -> insert at step 0.
    let p0 = repo
        .record_progress(&funnel_id, &session_id, 0)
        .await
        .expect("record 0");
    assert_eq!(p0.current_step, 0);
    assert_eq!(p0.completed_at, None);
    assert_eq!(p0.step_timestamps.as_array().map(Vec::len), Some(1));

    // Advance forward -> appends a timestamp and updates current_step.
    let p2 = repo
        .record_progress(&funnel_id, &session_id, 2)
        .await
        .expect("record 2");
    assert_eq!(p2.current_step, 2);
    assert_eq!(p2.step_timestamps.as_array().map(Vec::len), Some(2));

    // Backward move is ignored: current_step stays at 2, timestamps unchanged.
    let p_back = repo
        .record_progress(&funnel_id, &session_id, 1)
        .await
        .expect("record 1 (backward)");
    assert_eq!(p_back.current_step, 2);
    assert_eq!(p_back.step_timestamps.as_array().map(Vec::len), Some(2));

    let persisted = repo
        .find_progress(&funnel_id, &session_id)
        .await
        .expect("find progress")
        .expect("present");
    assert_eq!(persisted.current_step, 2);

    repo.delete(&funnel_id).await.expect("cleanup");
}

#[tokio::test]
async fn mark_completed_sets_completed_at() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let created = repo
        .create_funnel(&sample_input(&unique_funnel_name()))
        .await
        .expect("create");
    let funnel_id = created.funnel.id.clone();
    let session_id = SessionId::new(format!("sess-{}", Uuid::new_v4()));

    // No progress row yet -> nothing to complete.
    let no_row = repo
        .mark_completed(&funnel_id, &session_id)
        .await
        .expect("mark completed (no row)");
    assert!(!no_row);

    repo.record_progress(&funnel_id, &session_id, 1)
        .await
        .expect("record");

    let completed = repo
        .mark_completed(&funnel_id, &session_id)
        .await
        .expect("mark completed");
    assert!(completed);

    let persisted = repo
        .find_progress(&funnel_id, &session_id)
        .await
        .expect("find")
        .expect("present");
    assert!(persisted.completed_at.is_some());

    repo.delete(&funnel_id).await.expect("cleanup");
}
