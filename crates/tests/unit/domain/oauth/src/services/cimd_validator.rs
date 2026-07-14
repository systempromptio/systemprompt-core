// ClientValidator dispatch over the four client-id shapes plus the CIMD
// fetch-failure arm; CimdFetcher network-failure arm via a non-resolvable
// HTTPS host.

use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_oauth::error::OauthError;
use systemprompt_oauth::models::cimd::ClientValidation;
use systemprompt_oauth::repository::{ClientRepository, CreateClientParams};
use systemprompt_oauth::services::cimd::{CimdFetcher, ClientValidator};
use systemprompt_oauth::services::hash_client_secret;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

async fn setup() -> Option<(systemprompt_database::DbPool, ClientValidator)> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let validator = ClientValidator::new(&pool).expect("validator");
    Some((pool, validator))
}

#[tokio::test]
async fn first_party_and_system_clients_validate_without_io() {
    let Some((_pool, validator)) = setup().await else {
        return;
    };

    let sp = ClientId::new("sp_first_party_client");
    let validation = validator.validate_client(&sp, None).await.expect("sp");
    assert!(matches!(
        validation,
        ClientValidation::FirstParty { client_id } if client_id == sp
    ));

    let sys = ClientId::new("sys_internal_service");
    let validation = validator.validate_client(&sys, None).await.expect("sys");
    assert!(matches!(
        validation,
        ClientValidation::System { client_id } if client_id == sys
    ));
}

#[tokio::test]
async fn unknown_client_id_shape_is_rejected() {
    let Some((_pool, validator)) = setup().await else {
        return;
    };

    let bogus = ClientId::new("totally-unrecognised-shape");
    let err = validator
        .validate_client(&bogus, None)
        .await
        .expect_err("unknown shape");
    assert!(matches!(err, OauthError::InvalidClientMetadata(_)));
    assert!(err.to_string().contains("Invalid client_id format"));
}

#[tokio::test]
async fn dcr_client_resolves_when_registered_and_errors_when_absent() {
    let Some((pool, validator)) = setup().await else {
        return;
    };
    let owner = unique_user_id("cimdval");
    seed_user_row(
        &pool,
        &owner,
        &format!("{}@cimdval.invalid", owner.as_str()),
    )
    .await
    .expect("seed owner");

    let client_id = ClientId::new(format!("client_{}", Uuid::new_v4().simple()));
    let repo = ClientRepository::new(&pool).expect("client repo");
    repo.create(CreateClientParams {
        client_id: client_id.clone(),
        owner_user_id: owner,
        client_secret_hash: hash_client_secret("cimd-validator-secret-32-chars-long")
            .expect("hash"),
        client_name: "cimd-validator-test".to_owned(),
        redirect_uris: vec!["http://127.0.0.1/cb".to_owned()],
        grant_types: Some(vec!["authorization_code".to_owned()]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: vec!["openid".to_owned()],
        token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
        application_type: "web".to_owned(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    })
    .await
    .expect("create client");

    let validation = validator
        .validate_client(&client_id, None)
        .await
        .expect("registered dcr client");
    assert!(matches!(
        validation,
        ClientValidation::Dcr { client_id: id } if id == client_id
    ));

    let missing = ClientId::new(format!("client_{}", Uuid::new_v4().simple()));
    let err = validator
        .validate_client(&missing, None)
        .await
        .expect_err("unregistered dcr client");
    assert!(matches!(err, OauthError::ClientNotFound(_)));
}

async fn seed_unknown_user(pool: &systemprompt_database::DbPool) -> UserId {
    let user = unique_user_id("cimdval2");
    seed_user_row(pool, &user, &format!("{}@cimdval.invalid", user.as_str()))
        .await
        .expect("seed");
    user
}

#[tokio::test]
async fn cimd_client_fetch_failure_propagates() {
    let Some((pool, validator)) = setup().await else {
        return;
    };
    let _ = seed_unknown_user(&pool).await;

    let unreachable = ClientId::new(format!(
        "https://cimd-{}.invalid/client-metadata.json",
        Uuid::new_v4().simple()
    ));
    let err = validator
        .validate_client(&unreachable, Some("https://app.example/cb"))
        .await
        .expect_err("unresolvable host");
    assert!(matches!(err, OauthError::CimdFetch(_)));
}

#[tokio::test]
async fn fetcher_reports_network_failure_with_url_context() {
    ensure_test_bootstrap();
    let fetcher = CimdFetcher::new().expect("fetcher");
    let client_id = ClientId::new(format!(
        "https://cimd-{}.invalid/metadata.json",
        Uuid::new_v4().simple()
    ));

    let err = fetcher
        .fetch_metadata(&client_id)
        .await
        .expect_err("dns failure");
    let msg = err.to_string();
    assert!(msg.contains("Failed to fetch CIMD metadata"));
    assert!(msg.contains(client_id.as_str()));
}
