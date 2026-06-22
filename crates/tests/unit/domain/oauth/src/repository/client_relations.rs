// DB-backed tests for client-to-relation join loading (single + batch paths).

use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_oauth::repository::{ClientRepository, CreateClientParams};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: ClientRepository,
    owner: UserId,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ClientRepository::new(&pool).expect("client repo");
    let owner = unique_user_id("rel-owner");
    seed_user_row(&pool, &owner, &format!("{}@rel.invalid", owner.as_str()))
        .await
        .expect("seed owner");
    Some(Ctx { repo, owner })
}

fn new_client_id() -> ClientId {
    ClientId::new(format!("rel-{}", Uuid::new_v4().simple()))
}

fn params_full(client_id: &ClientId, owner: &UserId) -> CreateClientParams {
    CreateClientParams {
        client_id: client_id.clone(),
        owner_user_id: owner.clone(),
        client_secret_hash: "hash".to_owned(),
        client_name: "rel-full".to_owned(),
        redirect_uris: vec![
            "https://r.invalid/primary".to_owned(),
            "https://r.invalid/secondary".to_owned(),
            "https://r.invalid/third".to_owned(),
        ],
        grant_types: Some(vec![
            "authorization_code".to_owned(),
            "refresh_token".to_owned(),
        ]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: vec![
            "openid".to_owned(),
            "profile".to_owned(),
            "email".to_owned(),
        ],
        token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
        application_type: "web".to_owned(),
        client_uri: Some("https://r.invalid".to_owned()),
        logo_uri: None,
        contacts: Some(vec!["a@r.invalid".to_owned(), "b@r.invalid".to_owned()]),
    }
}

fn params_no_contacts(client_id: &ClientId, owner: &UserId) -> CreateClientParams {
    CreateClientParams {
        client_id: client_id.clone(),
        owner_user_id: owner.clone(),
        client_secret_hash: "hash".to_owned(),
        client_name: "rel-no-contacts".to_owned(),
        redirect_uris: vec!["https://nc.invalid/cb".to_owned()],
        grant_types: Some(vec!["authorization_code".to_owned()]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: vec!["openid".to_owned()],
        token_endpoint_auth_method: Some("none".to_owned()),
        application_type: "native".to_owned(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    }
}

#[tokio::test]
async fn single_load_returns_all_relation_cardinalities() {
    let Some(ctx) = setup().await else { return };
    let client_id = new_client_id();
    ctx.repo
        .create(params_full(&client_id, &ctx.owner))
        .await
        .expect("create");

    let found = ctx
        .repo
        .get_by_client_id(&client_id)
        .await
        .expect("get")
        .expect("present");

    assert_eq!(found.redirect_uris.len(), 3);
    assert_eq!(found.grant_types.len(), 2);
    assert!(found.grant_types.contains(&"refresh_token".to_owned()));
    assert_eq!(found.response_types, vec!["code".to_owned()]);
    assert_eq!(found.scopes.len(), 3);
    assert!(found.scopes.contains(&"email".to_owned()));
    let contacts = found.contacts.as_ref().expect("contacts present");
    assert_eq!(contacts.len(), 2);
}

#[tokio::test]
async fn single_load_absent_contacts_is_none() {
    let Some(ctx) = setup().await else { return };
    let client_id = new_client_id();
    ctx.repo
        .create(params_no_contacts(&client_id, &ctx.owner))
        .await
        .expect("create");

    let found = ctx
        .repo
        .get_by_client_id(&client_id)
        .await
        .expect("get")
        .expect("present");

    assert!(
        found.contacts.is_none(),
        "a client with no contact rows should load None contacts"
    );
    assert_eq!(found.redirect_uris.len(), 1);
    assert_eq!(found.scopes, vec!["openid".to_owned()]);
}

#[tokio::test]
async fn batch_load_distributes_relations_per_client() {
    let Some(ctx) = setup().await else { return };
    let with_contacts = new_client_id();
    let without_contacts = new_client_id();
    ctx.repo
        .create(params_full(&with_contacts, &ctx.owner))
        .await
        .expect("create full");
    ctx.repo
        .create(params_no_contacts(&without_contacts, &ctx.owner))
        .await
        .expect("create no-contacts");

    let all = ctx.repo.list().await.expect("list");

    let full = all
        .iter()
        .find(|c| c.client_id == with_contacts)
        .expect("full client present in batch");
    assert_eq!(full.redirect_uris.len(), 3);
    assert_eq!(full.scopes.len(), 3);
    assert_eq!(full.contacts.as_ref().map(Vec::len), Some(2));

    let lean = all
        .iter()
        .find(|c| c.client_id == without_contacts)
        .expect("lean client present in batch");
    assert_eq!(lean.redirect_uris.len(), 1);
    assert_eq!(lean.grant_types, vec!["authorization_code".to_owned()]);
    assert!(
        lean.contacts.is_none() || lean.contacts.as_ref().is_some_and(Vec::is_empty),
        "lean client must carry no contacts in batch load"
    );
}

#[tokio::test]
async fn batch_load_via_pagination_matches_single_load() {
    let Some(ctx) = setup().await else { return };
    let client_id = new_client_id();
    ctx.repo
        .create(params_full(&client_id, &ctx.owner))
        .await
        .expect("create");

    let single = ctx
        .repo
        .get_by_client_id(&client_id)
        .await
        .expect("single")
        .expect("present");

    let page = ctx.repo.list_paginated(500, 0).await.expect("page");
    let batched = page
        .iter()
        .find(|c| c.client_id == client_id)
        .expect("present in page");

    assert_eq!(single.redirect_uris.len(), batched.redirect_uris.len());
    assert_eq!(single.scopes.len(), batched.scopes.len());
    assert_eq!(single.grant_types.len(), batched.grant_types.len());
    assert_eq!(
        single.contacts.as_ref().map(Vec::len),
        batched.contacts.as_ref().map(Vec::len)
    );
}
