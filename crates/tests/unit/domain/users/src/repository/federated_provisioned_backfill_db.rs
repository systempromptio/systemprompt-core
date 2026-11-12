//! DB-backed tests for the users migration that records which links written
//! before the `provisioned` column were created by the issuer provisioning
//! their account, and the role ownership a sign-in reports afterwards.

use systemprompt_identifiers::UserId;
use systemprompt_traits::FederatedLink;

use super::federated_link_policy_db::{Ctx, LINK_ON, setup};

const BACKFILL_SQL: &str = include_str!(
    "../../../../../../domain/users/schema/migrations/034_federated_identities_provisioned_backfill.sql"
);

async fn seed_pre_column_link(ctx: &Ctx, user: &UserId, account_age: &str) {
    let pool = ctx.pool.pool();
    sqlx::query(
        "INSERT INTO federated_identities (issuer, external_sub, user_id, created_at, \
         last_seen_at, provisioned)
         SELECT $1, $2, $3, t, t, false
           FROM (SELECT COALESCE((SELECT applied_at FROM extension_migrations
                                   WHERE extension_id = 'users' AND version = 33), now())
                        - interval '1 day' AS t) s",
    )
    .bind(&ctx.issuer)
    .bind(&ctx.sub)
    .bind(user.as_str())
    .execute(pool.as_ref())
    .await
    .expect("seed link");
    sqlx::query(
        "UPDATE users u SET created_at = f.created_at + $2::interval
           FROM federated_identities f
          WHERE u.id = $1 AND f.user_id = u.id",
    )
    .bind(user.as_str())
    .bind(account_age)
    .execute(pool.as_ref())
    .await
    .expect("age account");
}

async fn run_backfill(ctx: &Ctx) {
    sqlx::raw_sql(BACKFILL_SQL)
        .execute(ctx.pool.pool().as_ref())
        .await
        .expect("backfill");
}

#[tokio::test]
async fn an_account_created_with_its_link_takes_the_issuers_roles_after_the_backfill() {
    let ctx = setup().await;
    let user = ctx.unverified().await;
    seed_pre_column_link(&ctx, &user.id, "40 milliseconds").await;

    run_backfill(&ctx).await;
    let (resolved, link) = ctx.resolve(&["admin"], &LINK_ON).await.expect("resolve");

    assert_eq!(resolved.id, user.id);
    assert_eq!(link, FederatedLink::Existing { provisioned: true });
    assert!(link.issuer_owns_roles());
    ctx.cleanup(&user.id).await;
}

#[tokio::test]
async fn an_account_that_predates_its_link_never_takes_the_issuers_roles() {
    let ctx = setup().await;
    let user = ctx.ordinary().await;
    seed_pre_column_link(&ctx, &user.id, "-30 days").await;

    run_backfill(&ctx).await;
    let (resolved, link) = ctx.resolve(&["admin"], &LINK_ON).await.expect("resolve");

    assert_eq!(resolved.id, user.id);
    assert_eq!(link, FederatedLink::Existing { provisioned: false });
    assert!(!link.issuer_owns_roles());
    ctx.cleanup(&user.id).await;
}

#[tokio::test]
async fn an_account_created_after_its_link_was_repointed_stays_not_provisioned() {
    let ctx = setup().await;
    let user = ctx.ordinary().await;
    seed_pre_column_link(&ctx, &user.id, "2 days").await;

    run_backfill(&ctx).await;
    let (_, link) = ctx.resolve(&["admin"], &LINK_ON).await.expect("resolve");

    assert_eq!(link, FederatedLink::Existing { provisioned: false });
    ctx.cleanup(&user.id).await;
}

#[tokio::test]
async fn a_link_written_once_the_column_existed_is_left_alone() {
    let ctx = setup().await;
    let user = ctx.unverified().await;
    let pool = ctx.pool.pool();
    sqlx::query(
        "INSERT INTO federated_identities (issuer, external_sub, user_id, provisioned)
         VALUES ($1, $2, $3, false)",
    )
    .bind(&ctx.issuer)
    .bind(&ctx.sub)
    .bind(user.id.as_str())
    .execute(pool.as_ref())
    .await
    .expect("seed link");
    sqlx::query("UPDATE users SET created_at = now() WHERE id = $1")
        .bind(user.id.as_str())
        .execute(pool.as_ref())
        .await
        .expect("age account");

    run_backfill(&ctx).await;

    assert_eq!(ctx.link_row().await.map(|(_, p)| p), Some(false));
    ctx.cleanup(&user.id).await;
}
