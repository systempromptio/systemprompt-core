// DB-backed ClientRepository CRUD + relation tests.

use systemprompt_identifiers::ClientId;
use systemprompt_oauth::repository::{ClientRepository, CreateClientParams, UpdateClientParams};
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
    let owner = unique_user_id("client-owner");
    seed_user_row(&pool, &owner, &format!("{}@client.invalid", owner.as_str()))
        .await
        .expect("seed owner");
    Some(Ctx { repo, owner })
}

fn create_params(client_id: &ClientId, owner: &systemprompt_identifiers::UserId) -> CreateClientParams {
    CreateClientParams {
        client_id: client_id.clone(),
        owner_user_id: owner.clone(),
        client_secret_hash: "hash-placeholder".to_owned(),
        client_name: "crud-client".to_owned(),
        redirect_uris: vec![
            "https://app.invalid/cb".to_owned(),
            "https://app.invalid/cb2".to_owned(),
        ],
        grant_types: Some(vec!["authorization_code".to_owned()]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: vec!["openid".to_owned(), "profile".to_owned()],
        token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
        application_type: "web".to_owned(),
        client_uri: Some("https://app.invalid".to_owned()),
        logo_uri: Some("https://app.invalid/logo.png".to_owned()),
        contacts: Some(vec!["admin@app.invalid".to_owned()]),
    }
}

#[tokio::test]
async fn create_then_get_by_client_id_loads_relations() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    let created = ctx
        .repo
        .create(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");
    assert_eq!(created.client_id, client_id);
    assert_eq!(created.scopes.len(), 2);

    let found = ctx
        .repo
        .get_by_client_id(&client_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(found.client_name, "crud-client");
    assert_eq!(found.redirect_uris.len(), 2);
    assert!(found.scopes.contains(&"openid".to_owned()));
    assert_eq!(found.contacts.as_ref().map(Vec::len), Some(1));
    assert_eq!(found.client_uri.as_deref(), Some("https://app.invalid"));
}

#[tokio::test]
async fn get_missing_client_returns_none() {
    let Some(ctx) = setup().await else { return };
    let missing = ClientId::new(format!("missing-{}", Uuid::new_v4().simple()));
    assert!(ctx
        .repo
        .get_by_client_id(&missing)
        .await
        .expect("get")
        .is_none());
}

#[tokio::test]
async fn deactivate_hides_from_active_get_but_visible_to_any() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    let n = ctx.repo.deactivate(&client_id).await.expect("deactivate");
    assert_eq!(n, 1);
    assert!(ctx
        .repo
        .get_by_client_id(&client_id)
        .await
        .expect("active get")
        .is_none());
    assert!(ctx
        .repo
        .get_by_client_id_any(&client_id)
        .await
        .expect("any get")
        .is_some());

    let n = ctx.repo.activate(&client_id).await.expect("activate");
    assert_eq!(n, 1);
    assert!(ctx
        .repo
        .get_by_client_id(&client_id)
        .await
        .expect("active get")
        .is_some());
}

#[tokio::test]
async fn update_replaces_relations() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    let params = UpdateClientParams {
        client_id: client_id.clone(),
        client_name: "renamed".to_owned(),
        redirect_uris: vec!["https://new.invalid/cb".to_owned()],
        grant_types: Some(vec!["authorization_code".to_owned(), "refresh_token".to_owned()]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: vec!["openid".to_owned()],
        token_endpoint_auth_method: Some("none".to_owned()),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };
    let updated = ctx.repo.update(params).await.expect("update").expect("present");
    assert_eq!(updated.client_name, "renamed");
    assert_eq!(updated.redirect_uris, vec!["https://new.invalid/cb".to_owned()]);
    assert_eq!(updated.scopes, vec!["openid".to_owned()]);
    assert!(updated.contacts.is_none() || updated.contacts.as_ref().is_some_and(Vec::is_empty));
}

#[tokio::test]
async fn update_missing_returns_none() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("missing-{}", Uuid::new_v4().simple()));
    let params = UpdateClientParams {
        client_id,
        client_name: "x".to_owned(),
        redirect_uris: vec!["https://x.invalid/cb".to_owned()],
        grant_types: None,
        response_types: None,
        scopes: vec!["openid".to_owned()],
        token_endpoint_auth_method: None,
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };
    assert!(ctx.repo.update(params).await.expect("update").is_none());
}

#[tokio::test]
async fn update_secret_changes_hash() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    let updated = ctx
        .repo
        .update_secret(&client_id, "new-hash-value")
        .await
        .expect("update_secret")
        .expect("present");
    assert_eq!(updated.client_id, client_id);

    let missing = ClientId::new(format!("missing-{}", Uuid::new_v4().simple()));
    assert!(ctx
        .repo
        .update_secret(&missing, "h")
        .await
        .expect("update_secret missing")
        .is_none());
}

#[tokio::test]
async fn delete_removes_row() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    let n = ctx.repo.delete(&client_id).await.expect("delete");
    assert_eq!(n, 1);
    assert!(ctx
        .repo
        .get_by_client_id_any(&client_id)
        .await
        .expect("get")
        .is_none());

    let n = ctx.repo.delete(&client_id).await.expect("delete again");
    assert_eq!(n, 0);
}

#[tokio::test]
async fn list_and_count_include_created_client() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    let count = ctx.repo.count().await.expect("count");
    assert!(count >= 1);
    let all = ctx.repo.list().await.expect("list");
    assert!(all.iter().any(|c| c.client_id == client_id));
    let page = ctx.repo.list_paginated(100, 0).await.expect("page");
    assert!(page.iter().any(|c| c.client_id == client_id));
}

#[tokio::test]
async fn find_by_redirect_uri_and_scope() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    let uri = format!("https://ru-{}.invalid/cb", Uuid::new_v4().simple());
    let mut params = create_params(&client_id, &ctx.owner);
    params.redirect_uris = vec![uri.clone()];
    ctx.repo.create(params).await.expect("create");

    let found = ctx
        .repo
        .find_by_redirect_uri(&uri)
        .await
        .expect("find")
        .expect("present");
    assert_eq!(found.client_id, client_id);

    let with_scope = ctx
        .repo
        .find_by_redirect_uri_with_scope(&uri, &["openid"])
        .await
        .expect("find scope")
        .expect("present");
    assert_eq!(with_scope.client_id, client_id);

    assert!(ctx
        .repo
        .find_by_redirect_uri_with_scope(&uri, &["does-not-exist"])
        .await
        .expect("find missing scope")
        .is_none());

    assert!(ctx
        .repo
        .find_by_redirect_uri("https://nobody.invalid/cb")
        .await
        .expect("find unknown")
        .is_none());
}

#[tokio::test]
async fn update_last_used_marks_timestamp() {
    let Some(ctx) = setup().await else { return };
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(create_params(&client_id, &ctx.owner))
        .await
        .expect("create");

    let ts = chrono::Utc::now().timestamp();
    ctx.repo
        .update_last_used(&client_id, ts)
        .await
        .expect("update_last_used");

    // After marking used, it should no longer appear in the unused list.
    let unused = ctx.repo.list_unused(0).await.expect("list_unused");
    assert!(!unused.iter().any(|c| c.client_id == client_id));
}
