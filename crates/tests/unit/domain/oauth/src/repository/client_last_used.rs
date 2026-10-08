// DB-backed ClientRepository last-use stamping tests.

use systemprompt_identifiers::ClientId;
use systemprompt_oauth::repository::{ClientRepository, CreateClientParams};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: ClientRepository,
    owner: systemprompt_identifiers::UserId,
}

async fn setup() -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = ClientRepository::new(&pool);
    let owner = unique_user_id("cleanup-owner");
    seed_user_row(
        &pool,
        &owner,
        &format!("{}@cleanup.invalid", owner.as_str()),
    )
    .await
    .expect("seed owner");
    Ctx { repo, owner }
}

async fn make_client(ctx: &Ctx) -> ClientId {
    let client_id = ClientId::new(format!("c-{}", Uuid::new_v4().simple()));
    ctx.repo
        .create(CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: ctx.owner.clone(),
            client_secret_hash: Some("hash".to_owned()),
            registration_token_hash: None,
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
async fn update_last_used_rejects_an_unrepresentable_timestamp() {
    let ctx = setup().await;
    let client_id = make_client(&ctx).await;
    assert!(
        ctx.repo
            .update_last_used(&client_id, i64::MAX)
            .await
            .is_err()
    );
}
