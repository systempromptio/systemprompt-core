//! DB-backed tests for CLI session context revalidation: a valid
//! context/user pairing is kept, a stale one is replaced with a recovered
//! CLI context owned by the session user.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_cli::session::resolution::helpers::revalidate_context;
use systemprompt_cloud::{CliSession, SessionIdentity};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, seed_user_session, unique_user_id,
};

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

async fn seeded_identity(pool: &DbPool, prefix: &str) -> (UserId, SessionId) {
    let user_id = unique_user_id(prefix);
    let email = format!("{}@test.local", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    let session_id = SessionId::generate();
    seed_user_session(pool, &user_id, &session_id).await.unwrap();
    (user_id, session_id)
}

fn session_for(user_id: &UserId, session_id: SessionId, context_id: ContextId) -> CliSession {
    CliSession::builder(
        ProfileName::new("ctxdb"),
        SessionToken::new("token"),
        session_id,
        context_id,
        SessionIdentity::new(
            user_id.clone(),
            Email::new("ctxdb@test.local"),
            UserType::Admin,
        ),
    )
    .build()
}

#[tokio::test]
async fn revalidate_context_keeps_a_context_owned_by_the_user() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxvalid").await;
    let repo = ContextRepository::new(&pool).unwrap();
    let context_id = repo
        .get_or_create_cli_context(&user_id, &session_id, "CLI Session - ctxdb")
        .await
        .unwrap();

    let mut session = session_for(&user_id, session_id, context_id.clone());
    let refreshed = revalidate_context(&pool, &mut session, "ctxdb").await;

    assert!(refreshed.is_none());
    assert_eq!(session.context_id, context_id);
}

#[tokio::test]
async fn revalidate_context_recovers_a_stale_context() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxstale").await;

    let stale = ContextId::generate();
    let mut session = session_for(&user_id, session_id, stale.clone());
    let refreshed = revalidate_context(&pool, &mut session, "ctxdb")
        .await
        .expect("stale context should be recovered");

    assert_ne!(session.context_id, stale);
    assert_eq!(refreshed.context_id, session.context_id);

    ContextRepository::new(&pool)
        .unwrap()
        .validate_context_ownership(&session.context_id, &user_id)
        .await
        .unwrap();
}

#[tokio::test]
async fn revalidate_context_adopts_the_existing_cli_context_by_name() {
    let pool = pool().await;
    let (user_id, session_id) = seeded_identity(&pool, "ctxadopt").await;
    let repo = ContextRepository::new(&pool).unwrap();
    let existing = repo
        .get_or_create_cli_context(&user_id, &session_id, "CLI Session - ctxdb")
        .await
        .unwrap();

    let mut session = session_for(&user_id, session_id, ContextId::generate());
    revalidate_context(&pool, &mut session, "ctxdb")
        .await
        .expect("stale context should be recovered");

    assert_eq!(session.context_id, existing);
}
