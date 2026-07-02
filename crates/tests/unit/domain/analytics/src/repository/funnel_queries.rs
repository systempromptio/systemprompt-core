//! DB-backed tests for `FunnelRepository` reads: finders (by name, active and
//! full listings, progress lookup), conversion statistics, and the
//! `FunnelMatchType` string round-trip.

use chrono::{Duration, Utc};
use systemprompt_analytics::{
    CreateFunnelInput, CreateFunnelStepInput, FunnelMatchType, FunnelRepository,
};
use systemprompt_identifiers::{FunnelId, SessionId};
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

fn two_step_input(name: &str) -> CreateFunnelInput {
    CreateFunnelInput {
        name: name.to_owned(),
        description: None,
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
        ],
    }
}

#[tokio::test]
async fn finders_locate_funnels_by_name_and_listing() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let name = format!("funnel-{}", Uuid::new_v4());
    let created = repo
        .create_funnel(&two_step_input(&name))
        .await
        .expect("create");
    let id = created.funnel.id.clone();

    let by_name = repo
        .find_by_name(&name)
        .await
        .expect("find_by_name")
        .expect("present");
    assert_eq!(by_name.funnel.id, id);
    assert_eq!(by_name.steps.len(), 2);

    assert!(
        repo.find_by_name("no-such-funnel")
            .await
            .expect("find")
            .is_none()
    );

    let active = repo.list_active().await.expect("list_active");
    assert!(active.iter().any(|f| f.id == id));

    assert!(repo.deactivate(&id).await.expect("deactivate"));

    let active_after = repo.list_active().await.expect("list_active again");
    assert!(!active_after.iter().any(|f| f.id == id));

    let all = repo.list_all().await.expect("list_all");
    assert!(all.iter().any(|f| f.id == id && !f.is_active));

    repo.delete(&id).await.expect("cleanup");
}

#[tokio::test]
async fn find_progress_returns_recorded_row() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let name = format!("funnel-{}", Uuid::new_v4());
    let created = repo
        .create_funnel(&two_step_input(&name))
        .await
        .expect("create");
    let id = created.funnel.id.clone();
    let session_id = SessionId::new(format!("sess-{}", Uuid::new_v4()));

    let missing = repo
        .find_progress(&id, &session_id)
        .await
        .expect("find_progress before");
    assert!(missing.is_none());

    repo.record_progress(&id, &session_id, 1)
        .await
        .expect("record");

    let progress = repo
        .find_progress(&id, &session_id)
        .await
        .expect("find_progress after")
        .expect("present");
    assert_eq!(progress.current_step, 1);
    assert_eq!(progress.funnel_id, id);
    assert!(progress.completed_at.is_none());

    repo.delete(&id).await.expect("cleanup");
}

#[tokio::test]
async fn get_stats_reports_entries_completions_and_steps() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let name = format!("funnel-{}", Uuid::new_v4());
    let created = repo
        .create_funnel(&two_step_input(&name))
        .await
        .expect("create");
    let id = created.funnel.id.clone();

    let finisher = SessionId::new(format!("sess-{}", Uuid::new_v4()));
    let dropper = SessionId::new(format!("sess-{}", Uuid::new_v4()));
    repo.record_progress(&id, &finisher, 1).await.expect("p1");
    repo.mark_completed(&id, &finisher).await.expect("complete");
    repo.record_progress(&id, &dropper, 0).await.expect("p2");

    let since = Utc::now() - Duration::hours(1);
    let stats = repo.get_stats(&id, since).await.expect("stats");

    assert_eq!(stats.funnel_id, id);
    assert_eq!(stats.funnel_name, name);
    assert_eq!(stats.total_entries, 2);
    assert_eq!(stats.total_completions, 1);
    assert!((stats.overall_conversion_rate - 50.0).abs() < f64::EPSILON);
    assert_eq!(stats.step_stats.len(), 2);
    assert_eq!(stats.step_stats[0].step_order, 0);
    assert_eq!(stats.step_stats[0].entered_count, 2);

    repo.delete(&id).await.expect("cleanup");
}

#[tokio::test]
async fn get_stats_for_unknown_funnel_is_an_error() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = FunnelRepository::new(&pool).expect("repo");

    let missing = FunnelId::new(format!("funnel-{}", Uuid::new_v4()));
    let err = repo
        .get_stats(&missing, Utc::now() - Duration::hours(1))
        .await
        .expect_err("missing funnel");
    assert!(err.to_string().contains(missing.as_str()));
}

#[test]
fn funnel_match_type_string_round_trip() {
    let variants = [
        FunnelMatchType::UrlExact,
        FunnelMatchType::UrlPrefix,
        FunnelMatchType::UrlRegex,
        FunnelMatchType::EventType,
    ];
    for variant in variants {
        assert_eq!(FunnelMatchType::parse_type(variant.as_str()), variant);
    }
    assert_eq!(
        FunnelMatchType::parse_type("anything-else"),
        FunnelMatchType::UrlPrefix
    );
}
