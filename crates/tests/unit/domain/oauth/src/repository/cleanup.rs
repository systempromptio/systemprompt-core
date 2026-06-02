// DB-backed tests for OAuthRepository client-cleanup delegation methods.

use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_oauth::repository::{CreateClientParams, OAuthRepository};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    owner: UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let owner = unique_user_id("cl-owner");
    seed_user_row(&pool, &owner, &format!("{}@cl.invalid", owner.as_str()))
        .await
        .expect("seed owner");
    Some(Ctx { repo, owner })
}

async fn make_client(ctx: &Ctx) -> ClientId {
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create_client(CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: ctx.owner.clone(),
            client_secret_hash: "hash".to_owned(),
            client_name: "cl".to_owned(),
            redirect_uris: vec!["https://cl.invalid/cb".to_owned()],
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
async fn cleanup_unused_clients_executes() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    assert!(
        ctx.repo
            .find_client_by_id(&client_id)
            .await
            .expect("find")
            .is_some()
    );
    ctx.repo
        .cleanup_unused_clients(0)
        .await
        .expect("cleanup_unused");
}

#[tokio::test]
async fn list_unused_and_old_clients() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;

    let _ = client_id;
    ctx.repo.list_unused_clients(0).await.expect("list_unused");
    ctx.repo.list_old_clients(0).await.expect("list_old");
}

#[tokio::test]
async fn stale_clients_after_mark_used() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    ctx.repo
        .update_client_last_used(&client_id)
        .await
        .expect("mark used");

    ctx.repo.list_stale_clients(0).await.expect("list_stale");
    ctx.repo
        .cleanup_stale_clients(0)
        .await
        .expect("cleanup_stale");
}

#[tokio::test]
async fn inactive_clients_lifecycle() {
    let Some(ctx) = setup().await else { return };
    let client_id = make_client(&ctx).await;
    // Deactivate via update path is unavailable on the façade; use delete_client
    // is not deactivation. Instead exercise list/cleanup of inactive which are
    // empty-safe even with no inactive rows.
    let _ = ctx
        .repo
        .list_inactive_clients()
        .await
        .expect("list_inactive");
    let _ = ctx
        .repo
        .cleanup_inactive_clients()
        .await
        .expect("cleanup_inactive");
    // The active client is still findable.
    assert!(
        ctx.repo
            .find_client_by_id(&client_id)
            .await
            .expect("find")
            .is_some()
    );
}

#[tokio::test]
async fn cleanup_and_deactivate_old_test_clients_run() {
    let Some(ctx) = setup().await else { return };
    // These target `test_%`-prefixed client ids; our ids are `c-...`, so the
    // calls are no-ops but still exercise the delegation path.
    let _ = ctx
        .repo
        .cleanup_old_test_clients(0)
        .await
        .expect("cleanup_old_test");
    let _ = ctx
        .repo
        .deactivate_old_test_clients(0)
        .await
        .expect("deactivate_old_test");
}
