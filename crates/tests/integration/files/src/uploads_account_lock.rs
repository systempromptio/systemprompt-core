use std::time::Duration;

use sqlx::PgConnection;
use systemprompt_files::uploads::UPLOAD_ACCOUNT_LOCK_CLASS;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::test_db_pool;

use crate::uploads_rig::{owner, request, rig, trace};

async fn admission_waits_while(hold: &'static str, user: &UserId) -> bool {
    let db = test_db_pool().await;
    let mut holder = db.pool().acquire().await.expect("holder connection");
    let holder: &mut PgConnection = &mut holder;
    sqlx::query(hold)
        .bind(UPLOAD_ACCOUNT_LOCK_CLASS)
        .bind(user.as_str().to_owned())
        .execute(&mut *holder)
        .await
        .expect("hold lock");
    let rig = rig().await;
    let admitted = tokio::time::timeout(
        Duration::from_millis(750),
        rig.service
            .create(user, request("held.txt", "text/plain", b"x"), &trace()),
    )
    .await;
    sqlx::query("SELECT pg_advisory_unlock_all()")
        .execute(&mut *holder)
        .await
        .expect("release");
    admitted.is_err()
}

#[tokio::test]
async fn admission_waits_for_the_namespaced_account_lock() {
    let user = owner();
    assert!(
        admission_waits_while("SELECT pg_advisory_lock($1, hashtext($2))", &user).await,
        "admission must serialise on the upload account class key"
    );
}

#[tokio::test]
async fn admission_waits_for_the_single_key_account_lock_of_the_previous_release() {
    let user = owner();
    assert!(
        admission_waits_while(
            "SELECT pg_advisory_lock(hashtext($2)::bigint) FROM (SELECT $1::int) AS unused",
            &user
        )
        .await,
        "a replica on the previous release still excludes this one"
    );
}
