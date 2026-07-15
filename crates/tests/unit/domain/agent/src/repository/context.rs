use super::{seed_user_and_session, try_pool};
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::ContextRepository;
use systemprompt_identifiers::{ContextId, SessionId, UserId};

async fn ctx_repo(pool: &systemprompt_database::DbPool) -> ContextRepository {
    ContextRepository::new(pool).expect("context repo")
}

#[tokio::test]
async fn create_get_and_validate_ownership() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;

    let context_id = repo
        .create_context(&user_id, Some(&session_id), "my-context", ContextKind::User)
        .await
        .expect("create");

    let ctx = repo.get_context(&context_id, &user_id).await.expect("get");
    assert_eq!(ctx.context_id, context_id);
    assert_eq!(ctx.user_id, user_id);
    assert_eq!(ctx.name, "my-context");

    repo.validate_context_ownership(&context_id, &user_id)
        .await
        .expect("owned");
}

#[tokio::test]
async fn create_without_session() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, _session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;

    let context_id = repo
        .create_context(&user_id, None, "no-session", ContextKind::User)
        .await
        .expect("create");
    let found = repo.get_context(&context_id, &user_id).await.expect("get");
    assert_eq!(found.name, "no-session");
}

#[tokio::test]
async fn get_context_wrong_user_is_not_found() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    let context_id = repo
        .create_context(&user_id, Some(&session_id), "ctx", ContextKind::User)
        .await
        .expect("create");

    let other = UserId::new("nonexistent-user");
    let err = repo.get_context(&context_id, &other).await.unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound(_)
    ));

    let err = repo
        .validate_context_ownership(&context_id, &other)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound(_)
    ));
}

#[tokio::test]
async fn find_user_id_for_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    let context_id = repo
        .create_context(&user_id, Some(&session_id), "ctx", ContextKind::User)
        .await
        .expect("create");

    let found = repo
        .find_user_id_for_context(&context_id)
        .await
        .expect("find");
    assert_eq!(found, Some(user_id));

    let missing = repo
        .find_user_id_for_context(&ContextId::generate())
        .await
        .expect("find missing");
    assert_eq!(missing, None);
}

#[tokio::test]
async fn find_by_session_id_returns_latest() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    repo.create_context(&user_id, Some(&session_id), "by-session", ContextKind::User)
        .await
        .expect("create");

    let found = repo
        .find_by_session_id(&session_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.user_id, user_id);

    let none = repo
        .find_by_session_id(&SessionId::generate())
        .await
        .expect("find missing");
    assert!(none.is_none());
}

#[tokio::test]
async fn update_context_name() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    let context_id = repo
        .create_context(&user_id, Some(&session_id), "before", ContextKind::User)
        .await
        .expect("create");

    repo.update_context_name(&context_id, &user_id, "after")
        .await
        .expect("rename");
    let ctx = repo.get_context(&context_id, &user_id).await.expect("get");
    assert_eq!(ctx.name, "after");
}

#[tokio::test]
async fn update_context_name_unknown_is_not_found() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, _session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    let err = repo
        .update_context_name(&ContextId::generate(), &user_id, "x")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound(_)
    ));
}

#[tokio::test]
async fn delete_context() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    let context_id = repo
        .create_context(&user_id, Some(&session_id), "doomed", ContextKind::User)
        .await
        .expect("create");

    repo.delete_context(&context_id, &user_id)
        .await
        .expect("delete");
    assert!(repo.get_context(&context_id, &user_id).await.is_err());

    let err = repo
        .delete_context(&context_id, &user_id)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound(_)
    ));
}

#[tokio::test]
async fn list_contexts_basic_and_with_stats() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    let c1 = repo
        .create_context(&user_id, Some(&session_id), "one", ContextKind::User)
        .await
        .expect("create");
    let c2 = repo
        .create_context(&user_id, Some(&session_id), "two", ContextKind::User)
        .await
        .expect("create");

    let basic = repo.list_contexts_basic(&user_id).await.expect("basic");
    assert!(basic.iter().any(|c| c.context_id == c1));
    assert!(basic.iter().any(|c| c.context_id == c2));

    let stats = repo
        .list_contexts_with_stats(&user_id)
        .await
        .expect("stats");
    let found = stats
        .iter()
        .find(|c| c.context_id == c1)
        .expect("c1 present");
    assert_eq!(found.task_count, 0);
    assert_eq!(found.message_count, 0);
}

#[tokio::test]
async fn get_context_events_since_empty() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;
    let context_id = repo
        .create_context(&user_id, Some(&session_id), "events", ContextKind::User)
        .await
        .expect("create");

    // Far-future cutoff: no updates after it.
    let future = chrono::Utc::now() + chrono::Duration::hours(1);
    let events = repo
        .get_context_events_since(&context_id, future)
        .await
        .expect("events");
    assert!(events.is_empty());
}

#[tokio::test]
async fn kind_round_trips_through_reads() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;

    let context_id = repo
        .create_context(
            &user_id,
            Some(&session_id),
            "CLI Session - test",
            ContextKind::CliSession,
        )
        .await
        .expect("create");

    let fetched = repo.get_context(&context_id, &user_id).await.expect("get");
    assert_eq!(fetched.kind, ContextKind::CliSession);

    let by_session = repo
        .find_by_session_id(&session_id)
        .await
        .expect("find by session")
        .expect("row present");
    assert_eq!(by_session.kind, ContextKind::CliSession);

    let listed = repo.list_contexts_basic(&user_id).await.expect("list");
    assert!(
        listed
            .iter()
            .all(|c| c.context_id != context_id || c.kind == ContextKind::CliSession)
    );
}

#[tokio::test]
async fn get_or_create_cli_context_reuses_row_across_sessions() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, first_session) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;

    let second_session = SessionId::generate();
    systemprompt_test_fixtures::seed_user_session(&pool, &user_id, &second_session)
        .await
        .expect("seed second session");

    let first = repo
        .get_or_create_cli_context(&user_id, &first_session, "CLI Session - local")
        .await
        .expect("first call");
    let second = repo
        .get_or_create_cli_context(&user_id, &second_session, "CLI Session - local")
        .await
        .expect("second call");
    assert_eq!(first, second, "same user+profile must reuse one row");

    let adopted = repo
        .find_by_session_id(&second_session)
        .await
        .expect("find by session")
        .expect("row re-pointed at the new session");
    assert_eq!(adopted.context_id, first);
    assert_eq!(adopted.kind, ContextKind::CliSession);
}

#[tokio::test]
async fn get_or_create_cli_context_keeps_profiles_separate() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let repo = ctx_repo(&pool).await;

    let a = repo
        .get_or_create_cli_context(&user_id, &session_id, "CLI Session - a")
        .await
        .expect("profile a");
    let b = repo
        .get_or_create_cli_context(&user_id, &session_id, "CLI Session - b")
        .await
        .expect("profile b");
    assert_ne!(a, b, "different profiles must not share a context row");
}
