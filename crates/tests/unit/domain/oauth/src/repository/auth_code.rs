// DB-backed authorization-code persistence tests (HMAC-at-rest store/consume,
// single-use, redirect-uri mismatch, PKCE S256).

use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};
use systemprompt_oauth::repository::{AuthCodeParams, OAuthRepository};
use systemprompt_test_fixtures::{
    OAuthClientFixture, PkcePair, ensure_test_bootstrap, fixture_database_url, fixture_db_pool,
    pkce_pair, seed_oauth_client, seed_user_row, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    client_id: ClientId,
    user_id: UserId,
    redirect_uri: String,
}

async fn setup() -> Option<Ctx> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = OAuthRepository::new(&pool).expect("repo");
    let user_id = unique_user_id("ac");
    seed_user_row(&pool, &user_id, &format!("{}@ac.invalid", user_id.as_str()))
        .await
        .expect("seed user");
    let OAuthClientFixture {
        client_id,
        redirect_uri,
        ..
    } = seed_oauth_client(&pool, &user_id)
        .await
        .expect("seed client");
    Some(Ctx {
        repo,
        client_id,
        user_id,
        redirect_uri,
    })
}

#[tokio::test]
async fn store_then_validate_without_pkce() {
    let Some(ctx) = setup().await else { return };
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid profile",
            code_challenge: None,
            code_challenge_method: None,
            resource: Some("https://api.invalid"),
        })
        .await
        .expect("store");

    let found_client = ctx
        .repo
        .get_client_id_from_auth_code(&code)
        .await
        .expect("client from code")
        .expect("present");
    assert_eq!(found_client, ctx.client_id);

    let result = ctx
        .repo
        .validate_authorization_code(&code, &ctx.client_id, Some(&ctx.redirect_uri), None)
        .await
        .expect("validate");
    assert_eq!(result.user_id, ctx.user_id);
    assert_eq!(result.scope, "openid profile");
    assert_eq!(result.resource.as_deref(), Some("https://api.invalid"));
}

#[tokio::test]
async fn validate_is_single_use() {
    let Some(ctx) = setup().await else { return };
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid",
            code_challenge: None,
            code_challenge_method: None,
            resource: None,
        })
        .await
        .expect("store");

    ctx.repo
        .validate_authorization_code(&code, &ctx.client_id, None, None)
        .await
        .expect("first use ok");

    // Second use is rejected (replay).
    assert!(
        ctx.repo
            .validate_authorization_code(&code, &ctx.client_id, None, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn validate_unknown_code_errors() {
    let Some(ctx) = setup().await else { return };
    let code = AuthorizationCode::new(format!("never-{}", Uuid::new_v4()));
    assert!(
        ctx.repo
            .validate_authorization_code(&code, &ctx.client_id, None, None)
            .await
            .is_err()
    );
    assert!(
        ctx.repo
            .get_client_id_from_auth_code(&code)
            .await
            .expect("lookup")
            .is_none()
    );
}

#[tokio::test]
async fn validate_redirect_uri_mismatch_errors() {
    let Some(ctx) = setup().await else { return };
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid",
            code_challenge: None,
            code_challenge_method: None,
            resource: None,
        })
        .await
        .expect("store");

    assert!(
        ctx.repo
            .validate_authorization_code(
                &code,
                &ctx.client_id,
                Some("https://evil.invalid/cb"),
                None
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn validate_pkce_s256_success_and_failure() {
    let Some(ctx) = setup().await else { return };
    let PkcePair {
        verifier,
        challenge,
        ..
    } = pkce_pair();

    // Wrong verifier rejected.
    let code1 = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code1,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid",
            code_challenge: Some(&challenge),
            code_challenge_method: Some("S256"),
            resource: None,
        })
        .await
        .expect("store");
    assert!(
        ctx.repo
            .validate_authorization_code(&code1, &ctx.client_id, None, Some("wrong-verifier"))
            .await
            .is_err()
    );

    // Correct verifier accepted (fresh code since the prior was consumed).
    let code2 = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code2,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid",
            code_challenge: Some(&challenge),
            code_challenge_method: Some("S256"),
            resource: None,
        })
        .await
        .expect("store");
    let ok = ctx
        .repo
        .validate_authorization_code(&code2, &ctx.client_id, None, Some(&verifier))
        .await
        .expect("pkce ok");
    assert_eq!(ok.user_id, ctx.user_id);
}

#[tokio::test]
async fn validate_pkce_missing_verifier_errors() {
    let Some(ctx) = setup().await else { return };
    let PkcePair { challenge, .. } = pkce_pair();
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid",
            code_challenge: Some(&challenge),
            code_challenge_method: Some("S256"),
            resource: None,
        })
        .await
        .expect("store");
    assert!(
        ctx.repo
            .validate_authorization_code(&code, &ctx.client_id, None, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn link_auth_code_to_dangling_refresh_token_errors() {
    let Some(ctx) = setup().await else { return };
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid",
            code_challenge: None,
            code_challenge_method: None,
            resource: None,
        })
        .await
        .expect("store");

    // refresh_token_id carries a foreign key into oauth_refresh_tokens, so
    // linking an id with no matching token row is rejected by the database.
    assert!(
        ctx.repo
            .link_auth_code_to_refresh_token(&code, "rt-id-value")
            .await
            .is_err()
    );
}
