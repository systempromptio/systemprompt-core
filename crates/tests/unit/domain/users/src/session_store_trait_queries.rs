use chrono::{Duration, Utc};
use systemprompt_identifiers::{SessionId, SessionSource, UserId};
use systemprompt_test_fixtures::DisposableDb;
use systemprompt_traits::SessionStore;
use systemprompt_traits::session_store::{CreateSessionParams, SessionIdentity, SessionNetwork};
use systemprompt_users::SessionRepository;

fn params<'a>(
    session_id: &'a SessionId,
    user_id: Option<&'a UserId>,
    fingerprint: &'a str,
) -> CreateSessionParams<'a> {
    CreateSessionParams {
        identity: SessionIdentity {
            session_id,
            user_id,
            session_source: SessionSource::Web,
            fingerprint_hash: Some(fingerprint),
        },
        network: SessionNetwork {
            ip_address: Some("203.0.113.9"),
            user_agent: Some("session-store-trait-queries"),
            ..SessionNetwork::default()
        },
        attribution: Default::default(),
        is_bot: false,
        is_ai_crawler: false,
        expires_at: Utc::now() + Duration::hours(1),
    }
}

async fn insert_user(pool: &systemprompt_database::DbPool, roles: &[&str]) -> UserId {
    let id = UserId::new(uuid::Uuid::new_v4().to_string());
    let roles: Vec<String> = roles.iter().map(|r| (*r).to_owned()).collect();
    sqlx::query("INSERT INTO users (id, name, email, roles) VALUES ($1, $2, $3, $4)")
        .bind(id.as_str())
        .bind(format!("store-{}", id.as_str()))
        .bind(format!("{}@test.invalid", id.as_str()))
        .bind(&roles)
        .execute(pool.write_pool().as_ref())
        .await
        .expect("insert user");
    id
}

#[tokio::test]
async fn fingerprint_and_geo_queries_find_a_fresh_anonymous_session() {
    let database = DisposableDb::with_schema("users_store_trait_anon").await;
    let pool = database.test_pool().await;
    let repository = SessionRepository::new(&pool);
    let store: &dyn SessionStore = &repository;
    let visitor = insert_user(&pool, &["anonymous"]).await;
    let session_id = SessionId::new(format!("anon-{}", uuid::Uuid::new_v4()));
    let fingerprint = format!("fp-{}", uuid::Uuid::new_v4());
    store
        .insert_session(&params(&session_id, Some(&visitor), &fingerprint))
        .await
        .expect("insert");

    assert_eq!(
        store
            .count_active_fingerprint_sessions(&fingerprint)
            .await
            .expect("count"),
        1
    );
    assert_eq!(
        store
            .find_reusable_fingerprint_session(&fingerprint)
            .await
            .expect("reusable"),
        Some(session_id.clone())
    );
    assert_eq!(
        store
            .count_active_fingerprint_sessions("fp-unknown")
            .await
            .expect("count unknown"),
        0
    );
    let recent = store
        .find_recent_by_fingerprint(&fingerprint, 3_600)
        .await
        .expect("recent")
        .expect("a recent anonymous session");
    assert_eq!(recent.session_id, session_id);
    assert_eq!(recent.user_id, Some(visitor.clone()));

    assert_eq!(
        store.count_sessions_missing_geo().await.expect("missing"),
        1
    );
    let missing = store
        .sessions_missing_geo(None, 10)
        .await
        .expect("missing geo");
    assert_eq!(
        missing,
        vec![(session_id.clone(), "203.0.113.9".to_owned())]
    );
    assert!(
        store
            .sessions_missing_geo(Some(&session_id), 10)
            .await
            .expect("after cursor")
            .is_empty()
    );
    store
        .set_session_geo(&session_id, Some("FR"), None, None)
        .await
        .expect("geo");
    assert_eq!(
        store.count_sessions_missing_geo().await.expect("missing"),
        0
    );

    assert_eq!(store.count_inactive(1).await.expect("inactive"), 0);
    assert_eq!(store.cleanup_inactive(1).await.expect("cleanup"), 0);
    let (requests, _) = store
        .get_session_velocity(&session_id)
        .await
        .expect("velocity");
    assert_eq!(requests, Some(0));
    assert!(store.find_by_id(&session_id).await.expect("find").is_some());

    pool.write_pool().close().await;
    database.drop_now().await;
}

#[tokio::test]
async fn user_bound_sessions_are_listed_found_and_revoked_together() {
    let database = DisposableDb::with_schema("users_store_trait_user").await;
    let pool = database.test_pool().await;
    let repository = SessionRepository::new(&pool);
    let store: &dyn SessionStore = &repository;
    let user = insert_user(&pool, &["user"]).await;
    let other = insert_user(&pool, &["user"]).await;
    let fingerprint = format!("fp-{}", uuid::Uuid::new_v4());
    let first = SessionId::new(format!("user-a-{}", uuid::Uuid::new_v4()));
    let second = SessionId::new(format!("user-b-{}", uuid::Uuid::new_v4()));
    for id in [&first, &second] {
        store
            .insert_session(&params(id, Some(&user), &fingerprint))
            .await
            .expect("insert");
    }

    let mut listed: Vec<SessionId> = store
        .list_active_by_user(&user)
        .await
        .expect("list")
        .into_iter()
        .map(|s| s.session_id)
        .collect();
    listed.sort();
    let mut expected = vec![first.clone(), second.clone()];
    expected.sort();
    assert_eq!(listed, expected);
    assert!(
        store
            .list_active_by_user(&other)
            .await
            .expect("other")
            .is_empty()
    );

    let found = store
        .find_by_fingerprint(&fingerprint, &user)
        .await
        .expect("by fingerprint")
        .expect("a session for the fingerprint");
    assert!(expected.contains(&found.session_id));
    assert!(
        store
            .find_by_fingerprint(&fingerprint, &other)
            .await
            .expect("other fingerprint")
            .is_none()
    );
    assert!(
        store
            .find_recent_by_fingerprint(&fingerprint, 3_600)
            .await
            .expect("recent")
            .is_none(),
        "only anonymous sessions are reused by fingerprint"
    );

    assert_eq!(store.revoke_all_for_user(&user).await.expect("revoke"), 2);
    assert_eq!(store.revoke_all_for_user(&user).await.expect("again"), 0);
    let revoked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM user_sessions WHERE user_id = $1 AND revoked_at IS NOT NULL",
    )
    .bind(user.as_str())
    .fetch_one(pool.write_pool().as_ref())
    .await
    .expect("revoked rows");
    assert_eq!(revoked, 2);

    assert_eq!(
        store
            .mark_no_js_sessions_as_bots(&[first.clone(), second.clone()])
            .await
            .expect("no-js"),
        0
    );
    assert_eq!(
        store.mark_ghost_sessions_as_bots().await.expect("ghosts"),
        0
    );
    let bots: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM user_sessions WHERE user_id = $1 AND is_bot")
            .bind(user.as_str())
            .fetch_one(pool.write_pool().as_ref())
            .await
            .expect("bot rows");
    assert_eq!(bots, 0);

    pool.write_pool().close().await;
    database.drop_now().await;
}
