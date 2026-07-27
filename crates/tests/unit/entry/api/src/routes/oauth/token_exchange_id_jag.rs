//! ID-JAG subject validation for token exchange.
//!
//! An ID-JAG is single-use: `validate_id_jag_subject` verifies the signature
//! and JOSE `typ`, applies the claim policy, and burns the `jti` so a replay is
//! refused. These tests cover the header rejections, the replay burn, and the
//! scope the identity carries out.

use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use systemprompt_api::routes::oauth::endpoints::token::generation::test_api::validate_id_jag_subject;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ClientId;
use systemprompt_models::Config;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::id_jag::ID_JAG_TYP;
use systemprompt_test_fixtures::{
    fixture_config, fixture_database_url, fixture_db_pool, install_test_signing_key,
};
use uuid::Uuid;

const CLIENT: &str = "sp_web";

fn config() -> Config {
    fixture_config(&fixture_database_url().unwrap())
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

/// Sign an ID-JAG with the authority key the fixture installs, so the
/// self-issued verification path resolves it by `kid`.
fn sign_id_jag(
    config: &Config,
    typ: Option<&str>,
    alg: Algorithm,
    claims: serde_json::Value,
) -> String {
    let key = install_test_signing_key();
    let mut header = Header::new(alg);
    header.kid = Some(key.kid().to_owned());
    header.typ = typ.map(ToOwned::to_owned);
    let der = key.private_key().to_pkcs1_der().expect("der");
    let _ = config;
    encode(&header, &claims, &EncodingKey::from_rsa_der(der.as_bytes())).expect("sign")
}

fn claims(config: &Config, jti: &str, scope: &str) -> serde_json::Value {
    let now = Utc::now().timestamp();
    serde_json::json!({
        "iss": config.jwt_issuer,
        "sub": "user-id-jag",
        "aud": config.jwt_issuer,
        "client_id": CLIENT,
        "jti": jti,
        "iat": now,
        "exp": now + 300,
        "scope": scope,
    })
}

async fn validate(token: &str, pool: &DbPool, config: &Config) -> anyhow::Result<Vec<String>> {
    let repo = OAuthRepository::new(pool).expect("oauth repo");
    validate_id_jag_subject(token, &ClientId::new(CLIENT), &repo, config)
        .await
        .map(|identity| {
            identity
                .scope
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
}

#[tokio::test]
async fn rejects_a_token_without_the_id_jag_typ() {
    let config = config();
    let pool = pool().await;
    let token = sign_id_jag(
        &config,
        Some("JWT"),
        Algorithm::RS256,
        claims(&config, &Uuid::new_v4().to_string(), "admin"),
    );

    let err = validate(&token, &pool, &config)
        .await
        .expect_err("a plain JWT typ must not pass as an ID-JAG");

    assert!(err.to_string().contains("typ"), "{err}");
}

#[tokio::test]
async fn rejects_a_token_that_is_not_rs256() {
    let config = config();
    let pool = pool().await;
    let key = install_test_signing_key();
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some(key.kid().to_owned());
    header.typ = Some(ID_JAG_TYP.to_owned());
    let token = encode(
        &header,
        &claims(&config, &Uuid::new_v4().to_string(), "admin"),
        &EncodingKey::from_secret(b"not-the-authority"),
    )
    .expect("sign");

    let err = validate(&token, &pool, &config)
        .await
        .expect_err("HS256 must be refused");

    assert!(err.to_string().contains("RS256"), "{err}");
}

#[tokio::test]
async fn rejects_an_issuer_that_is_not_trusted() {
    let mut config = config();
    let pool = pool().await;
    let mut body = claims(&config, &Uuid::new_v4().to_string(), "admin");
    body["iss"] = serde_json::json!("https://attacker.test");
    let token = sign_id_jag(&config, Some(ID_JAG_TYP), Algorithm::RS256, body);
    config.trusted_issuers.clear();

    let err = validate(&token, &pool, &config)
        .await
        .expect_err("an unknown issuer has no key to verify against");

    assert!(err.to_string().contains("is not trusted"), "{err}");
}

#[tokio::test]
async fn accepts_a_self_issued_id_jag_and_carries_its_scope() {
    let config = config();
    let pool = pool().await;
    let token = sign_id_jag(
        &config,
        Some(ID_JAG_TYP),
        Algorithm::RS256,
        claims(&config, &Uuid::new_v4().to_string(), "admin"),
    );

    let scope = validate(&token, &pool, &config)
        .await
        .expect("a well-formed self-issued ID-JAG validates");

    assert!(!scope.is_empty(), "the granted scope reaches the caller");
}

#[tokio::test]
async fn refuses_the_same_id_jag_twice() {
    let config = config();
    let pool = pool().await;
    let token = sign_id_jag(
        &config,
        Some(ID_JAG_TYP),
        Algorithm::RS256,
        claims(&config, &Uuid::new_v4().to_string(), "admin"),
    );

    validate(&token, &pool, &config)
        .await
        .expect("first use succeeds");
    let err = validate(&token, &pool, &config)
        .await
        .expect_err("an ID-JAG is single-use");

    assert!(err.to_string().contains("replay"), "{err}");
}
