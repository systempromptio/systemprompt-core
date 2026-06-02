// DB-backed tests for the OAuthRepository client façade (delegation to
// ClientRepository plus the façade-level validation in update_client).

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
    let owner = unique_user_id("facade-owner");
    seed_user_row(&pool, &owner, &format!("{}@facade.invalid", owner.as_str()))
        .await
        .expect("seed owner");
    Some(Ctx { repo, owner })
}

fn create_params(client_id: &ClientId, owner: &UserId) -> CreateClientParams {
    CreateClientParams {
        client_id: client_id.clone(),
        owner_user_id: owner.clone(),
        client_secret_hash: "hash".to_owned(),
        client_name: "facade".to_owned(),
        redirect_uris: vec!["https://f.invalid/cb".to_owned()],
        grant_types: Some(vec!["authorization_code".to_owned()]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: vec!["openid".to_owned()],
        token_endpoint_auth_method: Some("none".to_owned()),
        application_type: "web".to_owned(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    }
}

#[tokio::test]
async fn create_client_and_find_and_list_and_count() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    let created = ctx
        .repo
        .create_client(create_params(&client_id, &ctx.owner))
        .await
        .expect("create_client");
    assert_eq!(created.client_id, client_id);

    let found = ctx
        .repo
        .find_client_by_id(&client_id)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.client_name, "facade");

    assert!(ctx.repo.count_clients().await.expect("count") >= 1);
    assert!(
        ctx.repo
            .list_clients()
            .await
            .expect("list")
            .iter()
            .any(|c| c.client_id == client_id)
    );
    assert!(
        ctx.repo
            .list_clients_paginated(100, 0)
            .await
            .expect("paginated")
            .iter()
            .any(|c| c.client_id == client_id)
    );
}

#[tokio::test]
async fn update_client_replaces_fields() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create_client(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    let updated = ctx
        .repo
        .update_client(
            &client_id,
            Some("renamed"),
            Some(&["https://new.invalid/cb".to_owned()]),
            Some(&["openid".to_owned(), "profile".to_owned()]),
        )
        .await
        .expect("update_client");
    assert_eq!(updated.client_name, "renamed");
    assert_eq!(updated.scopes.len(), 2);
}

#[tokio::test]
async fn update_client_rejects_empty_name() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create_client(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    assert!(
        ctx.repo
            .update_client(
                &client_id,
                Some(""),
                Some(&["https://x.invalid/cb".to_owned()]),
                Some(&["openid".to_owned()]),
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn update_client_rejects_empty_redirect_uris() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create_client(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    assert!(
        ctx.repo
            .update_client(
                &client_id,
                Some("n"),
                Some(&[]),
                Some(&["openid".to_owned()])
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn update_client_rejects_empty_scopes() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create_client(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    assert!(
        ctx.repo
            .update_client(
                &client_id,
                Some("n"),
                Some(&["https://x.invalid/cb".to_owned()]),
                Some(&[]),
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn update_client_missing_errors() {
    let Some(ctx) = setup().await else { return };
    let missing = ClientId::new(format!("missing-{}", Uuid::new_v4().simple()));
    assert!(
        ctx.repo
            .update_client(
                &missing,
                Some("n"),
                Some(&["https://x.invalid/cb".to_owned()]),
                Some(&["openid".to_owned()]),
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn update_client_full_and_secret() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    let mut client = ctx
        .repo
        .create_client(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    client.client_name = "full-update".to_owned();
    let updated = ctx
        .repo
        .update_client_full(&client)
        .await
        .expect("update_client_full");
    assert_eq!(updated.client_name, "full-update");

    let with_secret = ctx
        .repo
        .update_client_secret(&client_id, "new-hash")
        .await
        .expect("update secret")
        .expect("present");
    assert_eq!(with_secret.client_id, client_id);
}

#[tokio::test]
async fn delete_client_returns_bool() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create_client(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    assert!(ctx.repo.delete_client(&client_id).await.expect("delete"));
    assert!(
        !ctx.repo
            .delete_client(&client_id)
            .await
            .expect("delete again")
    );
}

#[tokio::test]
async fn find_client_by_redirect_uri_facade() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    let uri = format!("https://ru-{}.invalid/cb", Uuid::new_v4().simple());
    let mut params = create_params(&client_id, &ctx.owner);
    params.redirect_uris = vec![uri.clone()];
    ctx.repo.create_client(params).await.expect("create");

    let found = ctx
        .repo
        .find_client_by_redirect_uri(&uri)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.client_id, client_id);

    let scoped = ctx
        .repo
        .find_client_by_redirect_uri_with_scope(&uri, &["openid"])
        .await
        .expect("find scoped")
        .expect("present");
    assert_eq!(scoped.client_id, client_id);
}
