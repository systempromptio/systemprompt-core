// DB-backed ClientRepository cleanup/listing tests.

use systemprompt_identifiers::ClientId;
use systemprompt_oauth::repository::{ClientRepository, CreateClientParams};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: ClientRepository,
    owner: systemprompt_identifiers::UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ClientRepository::new(&pool).expect("client repo");
    let owner = unique_user_id("cleanup-owner");
    seed_user_row(
        &pool,
        &owner,
        &format!("{}@cleanup.invalid", owner.as_str()),
    )
    .await
    .expect("seed owner");
    Some(Ctx { repo, owner })
}

async fn make_client(ctx: &Ctx) -> ClientId {
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: ctx.owner.clone(),
            client_secret_hash: "hash".to_owned(),
            client_name: "cleanup".to_owned(),
            redirect_uris: vec!["https://c.invalid/cb".to_owned()],
            grant_types: Some(vec!["authorization_code".to_owned()]),
            response_types: Some(vec!["code".to_owned()]),
            scopes: vec!["openid".to_owned()],
            token_endpoint_auth_method: Some("none".to_owned()),
            application_type: "web".to_owned(),
            client_uri: None,
            logo_uri: None,
            contacts: None,
        })
        .await
        .expect("create");
    client_id
}

#[tokio::test]
async fn cleanup_inactive_deletes_deactivated() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    ctx.repo.deactivate(&client_id).await.expect("deactivate");

    let removed = ctx.repo.cleanup_inactive().await.expect("cleanup_inactive");
    assert!(removed >= 1);
    assert!(
        ctx.repo
            .find_by_client_id_any(&client_id)
            .await
            .expect("get")
            .is_none()
    );
}

#[tokio::test]
async fn list_inactive_includes_deactivated() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    ctx.repo.deactivate(&client_id).await.expect("deactivate");

    let inactive = ctx.repo.list_inactive().await.expect("list_inactive");
    assert!(inactive.iter().any(|c| c.client_id == client_id));
}

#[tokio::test]
async fn delete_unused_removes_never_used() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;

    // never_used_before = -3600 means cutoff is one hour in the future, so a
    // just-created never-used client qualifies for deletion.
    let removed = ctx.repo.delete_unused(-3600).await.expect("delete_unused");
    assert!(removed >= 1);
    assert!(
        ctx.repo
            .find_by_client_id_any(&client_id)
            .await
            .expect("get")
            .is_none()
    );
}

#[tokio::test]
async fn list_unused_includes_never_used() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    let unused = ctx.repo.list_unused(-3600).await.expect("list_unused");
    assert!(unused.iter().any(|c| c.client_id == client_id));
    assert!(unused.iter().all(|c| c.last_used_at.is_none()));
}

#[tokio::test]
async fn list_old_includes_recent_with_future_cutoff() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    let future = chrono::Utc::now().timestamp() + 3600;
    let old = ctx.repo.list_old(future).await.expect("list_old");
    assert!(old.iter().any(|c| c.client_id == client_id));
}

#[tokio::test]
async fn list_stale_includes_recently_used() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    ctx.repo
        .update_last_used(&client_id, chrono::Utc::now().timestamp())
        .await
        .expect("update_last_used");

    // last_used_before = -3600 → cutoff in the future → recently-used qualifies.
    let stale = ctx.repo.list_stale(-3600).await.expect("list_stale");
    assert!(stale.iter().any(|c| c.client_id == client_id));
}

#[tokio::test]
async fn delete_stale_removes_recently_used() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    ctx.repo
        .update_last_used(&client_id, chrono::Utc::now().timestamp())
        .await
        .expect("update_last_used");

    let removed = ctx.repo.delete_stale(-3600).await.expect("delete_stale");
    assert!(removed >= 1);
}

#[tokio::test]
async fn list_old_rejects_invalid_timestamp() {
    let Some(ctx) = setup().await else { return };
    let err = ctx.repo.list_old(i64::MAX).await;
    assert!(err.is_err());
}
