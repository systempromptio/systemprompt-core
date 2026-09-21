use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_oauth::repository::{CreateClientParams, OAuthRepository};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use uuid::Uuid;

#[tokio::test]
async fn failed_contact_replacement_rolls_back_all_client_relations_then_retry_commits() {
    let database = DisposableDb::installed("oauth_contact_update_atomicity")
        .await
        .expect("isolated OAuth database");
    let pool = database.pool().await.expect("OAuth database pool");
    let owner = UserId::new(format!("oauth-owner-{}", Uuid::new_v4().simple()));
    seed_user_row(&pool, &owner, &format!("{}@oauth.invalid", owner.as_str()))
        .await
        .expect("seed OAuth owner");
    let repository = OAuthRepository::new(&pool).expect("OAuth repository");
    let client_id = ClientId::new(format!("atomic-{}", Uuid::new_v4().simple()));
    let original = repository
        .create_client(CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: owner,
            client_secret_hash: Some("original-secret".to_owned()),
            registration_token_hash: None,
            client_name: "original-client".to_owned(),
            redirect_uris: vec!["https://original.invalid/callback".to_owned()],
            grant_types: Some(vec!["authorization_code".to_owned()]),
            response_types: Some(vec!["code".to_owned()]),
            scopes: vec!["openid".to_owned()],
            token_endpoint_auth_method: Some("client_secret_post".to_owned()),
            application_type: "web".to_owned(),
            client_uri: None,
            logo_uri: None,
            contacts: Some(vec!["original@example.invalid".to_owned()]),
        })
        .await
        .expect("create original client");

    let raw = pool.write_pool_arc().expect("OAuth write pool");
    sqlx::query(
        "CREATE FUNCTION reject_oauth_contact() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'fixture contact association rejection'; END $$",
    )
    .execute(raw.as_ref())
    .await
    .expect("create contact fault function");
    sqlx::query(
        "CREATE TRIGGER reject_oauth_contact BEFORE INSERT ON oauth_client_contacts \
         FOR EACH ROW EXECUTE FUNCTION reject_oauth_contact()",
    )
    .execute(raw.as_ref())
    .await
    .expect("create contact fault trigger");

    let mut replacement = original.clone();
    replacement.client_name = "replacement-client".to_owned();
    replacement.redirect_uris = vec!["https://replacement.invalid/callback".to_owned()];
    replacement.grant_types = vec!["client_credentials".to_owned()];
    replacement.response_types = vec!["token".to_owned()];
    replacement.scopes = vec!["profile".to_owned()];
    replacement.contacts = Some(vec!["replacement@example.invalid".to_owned()]);

    let error = repository
        .update_client_full(&replacement)
        .await
        .expect_err("late contact association failure must abort the update");
    assert!(
        error
            .to_string()
            .contains("fixture contact association rejection"),
        "database diagnosis reaches the caller: {error}"
    );
    let retained = repository
        .find_client_by_id(&client_id)
        .await
        .expect("read retained client")
        .expect("client remains after rollback");
    assert_eq!(retained.client_name, "original-client");
    assert_eq!(retained.redirect_uris, original.redirect_uris);
    assert_eq!(retained.grant_types, original.grant_types);
    assert_eq!(retained.response_types, original.response_types);
    assert_eq!(retained.scopes, original.scopes);
    assert_eq!(retained.contacts, original.contacts);

    sqlx::query("DROP TRIGGER reject_oauth_contact ON oauth_client_contacts")
        .execute(raw.as_ref())
        .await
        .expect("remove contact fault trigger");
    let committed = repository
        .update_client_full(&replacement)
        .await
        .expect("retry client update");
    assert_eq!(committed.client_name, "replacement-client");
    assert_eq!(committed.redirect_uris, replacement.redirect_uris);
    assert_eq!(committed.grant_types, replacement.grant_types);
    assert_eq!(committed.response_types, replacement.response_types);
    assert_eq!(committed.scopes, replacement.scopes);
    assert_eq!(committed.contacts, replacement.contacts);
    let persisted = repository
        .find_client_by_id(&client_id)
        .await
        .expect("read committed replacement")
        .expect("replacement remains persisted");
    assert_eq!(persisted.client_name, replacement.client_name);
    assert_eq!(persisted.redirect_uris, replacement.redirect_uris);
    assert_eq!(persisted.grant_types, replacement.grant_types);
    assert_eq!(persisted.response_types, replacement.response_types);
    assert_eq!(persisted.scopes, replacement.scopes);
    assert_eq!(persisted.contacts, replacement.contacts);

    drop(repository);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
