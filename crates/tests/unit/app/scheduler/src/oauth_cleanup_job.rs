use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_oauth::jobs::OauthCleanupJob;
use systemprompt_provider_contracts::{Job, JobContext, ProviderError};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};

#[tokio::test]
async fn oauth_cleanup_job_requires_pool_and_removes_only_expired_rows() {
    let owner = UserId::new(format!("oauth-cleanup-{}", uuid::Uuid::new_v4().simple()));
    let actor = Actor::user(owner.clone());
    let missing = JobContext::new(actor.clone(), Arc::new(()), Arc::new(()), Arc::new(()));
    assert!(matches!(
        OauthCleanupJob.execute(&missing).await,
        Err(ProviderError::Configuration(message)) if message.contains("DbPool")
    ));

    let database = DisposableDb::installed("oauth_cleanup_job")
        .await
        .expect("private database");
    let db = database.pool().await.expect("private pool");
    seed_user_row(&db, &owner, &format!("{owner}@cleanup.invalid"))
        .await
        .expect("owner");
    let client = format!("cleanup-client-{}", uuid::Uuid::new_v4().simple());
    sqlx::query("INSERT INTO oauth_clients(client_id,client_name,owner_user_id) VALUES($1,$1,$2)")
        .bind(&client)
        .bind(owner.as_str())
        .execute(db.write_pool().as_ref())
        .await
        .expect("client");
    let expired = format!("expired-{}", uuid::Uuid::new_v4().simple());
    let live = format!("live-{}", uuid::Uuid::new_v4().simple());
    for (token, expires_at) in [
        (&expired, Utc::now() - Duration::hours(1)),
        (&live, Utc::now() + Duration::hours(1)),
    ] {
        sqlx::query(
            "INSERT INTO oauth_refresh_tokens(token_id,client_id,user_id,scope,expires_at,family_id) \
             VALUES($1,$2,$3,'openid',$4,$1)",
        )
        .bind(token)
        .bind(&client)
        .bind(owner.as_str())
        .bind(expires_at)
        .execute(db.write_pool().as_ref())
        .await
        .expect("refresh token");
    }
    let replay = format!("replay-{}", uuid::Uuid::new_v4().simple());
    sqlx::query("INSERT INTO id_jag_replay(jti,expires_at) VALUES($1,NOW()-INTERVAL '1 hour')")
        .bind(&replay)
        .execute(db.write_pool().as_ref())
        .await
        .expect("expired replay marker");

    let context = JobContext::new(actor, Arc::new(db.clone()), Arc::new(()), Arc::new(()));
    let result = OauthCleanupJob
        .execute(&context)
        .await
        .expect("cleanup job");
    assert!(result.success);
    assert_eq!(result.items_processed, Some(2));
    assert_eq!(result.items_failed, Some(0));
    let remaining: Vec<String> = sqlx::query_scalar(
        "SELECT token_id FROM oauth_refresh_tokens WHERE token_id IN ($1,$2) ORDER BY token_id",
    )
    .bind(&expired)
    .bind(&live)
    .fetch_all(db.write_pool().as_ref())
    .await
    .expect("remaining tokens");
    assert_eq!(remaining, vec![live]);
    let replay_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM id_jag_replay WHERE jti=$1)")
            .bind(&replay)
            .fetch_one(db.write_pool().as_ref())
            .await
            .expect("replay existence");
    assert!(!replay_exists);

    drop(context);
    drop(db);
    database.drop_now().await;
}
