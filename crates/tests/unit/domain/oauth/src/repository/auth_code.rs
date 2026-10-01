// DB-backed authorization-code persistence tests (HMAC-at-rest store/consume,
// single-use, mandatory redirect-uri and PKCE S256 checks).

use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId, UserId};
use systemprompt_oauth::repository::{AuthCodeParams, MintAuthCodeParams, OAuthRepository};
use systemprompt_test_fixtures::{
    OAuthClientFixture, PkcePair, ensure_test_bootstrap, pkce_pair, seed_oauth_client,
    seed_user_row, test_db_pool, unique_user_id,
};
use uuid::Uuid;

struct Ctx {
    repo: OAuthRepository,
    client_id: ClientId,
    user_id: UserId,
    redirect_uri: String,
    pkce: PkcePair,
}

async fn setup() -> Ctx {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = OAuthRepository::new(&pool);
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
    Ctx {
        repo,
        client_id,
        user_id,
        redirect_uri,
        pkce: pkce_pair(),
    }
}

async fn store_code(ctx: &Ctx, scope: &str, resource: Option<&str>) -> AuthorizationCode {
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope,
            code_challenge: &ctx.pkce.challenge,
            resource,
        })
        .await
        .expect("store");
    code
}

#[tokio::test]
async fn store_then_validate_with_pkce() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid profile", Some("https://api.invalid")).await;

    let found_client = ctx
        .repo
        .find_client_id_from_auth_code(&code)
        .await
        .expect("client from code")
        .expect("present");
    assert_eq!(found_client, ctx.client_id);

    let result = ctx
        .repo
        .validate_authorization_code(&code, &ctx.client_id, &ctx.redirect_uri, &ctx.pkce.verifier)
        .await
        .expect("validate");
    assert_eq!(result.user_id, ctx.user_id);
    assert_eq!(result.scope, "openid profile");
    assert_eq!(result.resource.as_deref(), Some("https://api.invalid"));
}

#[tokio::test]
async fn validate_is_single_use() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid", None).await;

    ctx.repo
        .validate_authorization_code(&code, &ctx.client_id, &ctx.redirect_uri, &ctx.pkce.verifier)
        .await
        .expect("first use ok");

    assert!(
        ctx.repo
            .validate_authorization_code(
                &code,
                &ctx.client_id,
                &ctx.redirect_uri,
                &ctx.pkce.verifier,
            )
            .await
            .is_err(),
        "a replayed code must be rejected"
    );
}

#[tokio::test]
async fn validate_unknown_code_errors() {
    let ctx = setup().await;
    let code = AuthorizationCode::new(format!("never-{}", Uuid::new_v4()));
    assert!(
        ctx.repo
            .validate_authorization_code(
                &code,
                &ctx.client_id,
                &ctx.redirect_uri,
                &ctx.pkce.verifier,
            )
            .await
            .is_err()
    );
    assert!(
        ctx.repo
            .find_client_id_from_auth_code(&code)
            .await
            .expect("lookup")
            .is_none()
    );
}

#[tokio::test]
async fn validate_redirect_uri_mismatch_errors() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid", None).await;

    assert!(
        ctx.repo
            .validate_authorization_code(
                &code,
                &ctx.client_id,
                "https://evil.invalid/cb",
                &ctx.pkce.verifier,
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn validate_rejects_mismatched_client_id() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid", None).await;

    let other_client = ClientId::new(format!("other-{}", Uuid::new_v4()));
    assert!(
        ctx.repo
            .validate_authorization_code(
                &code,
                &other_client,
                &ctx.redirect_uri,
                &ctx.pkce.verifier,
            )
            .await
            .is_err(),
        "a code issued to one client must not be redeemable by another"
    );

    assert!(
        ctx.repo
            .validate_authorization_code(
                &code,
                &ctx.client_id,
                &ctx.redirect_uri,
                &ctx.pkce.verifier,
            )
            .await
            .is_err(),
        "the mismatched attempt consumed the code, so the rightful client is refused too"
    );
}

#[tokio::test]
async fn validate_rejects_a_wrong_verifier() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid", None).await;
    assert!(
        ctx.repo
            .validate_authorization_code(
                &code,
                &ctx.client_id,
                &ctx.redirect_uri,
                "wrong-verifier",
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn validate_rejects_an_empty_verifier() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid", None).await;
    assert!(
        ctx.repo
            .validate_authorization_code(&code, &ctx.client_id, &ctx.redirect_uri, "")
            .await
            .is_err(),
        "a PKCE-bound code must not be redeemable without a verifier"
    );
}

#[tokio::test]
async fn store_rejects_an_empty_challenge() {
    let ctx = setup().await;
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4()));
    ctx.repo
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            redirect_uri: &ctx.redirect_uri,
            scope: "openid",
            code_challenge: "",
            resource: None,
        })
        .await
        .expect_err("a code without a PKCE challenge must not be stored");
}

#[tokio::test]
async fn mint_rejects_a_missing_challenge_or_a_non_s256_method() {
    let ctx = setup().await;
    let base = MintAuthCodeParams {
        client_id: &ctx.client_id,
        user_id: &ctx.user_id,
        redirect_uri: &ctx.redirect_uri,
        scope: Some("openid"),
        code_challenge: &ctx.pkce.challenge,
        code_challenge_method: "S256",
        resource: None,
    };

    ctx.repo
        .mint_authorization_code(MintAuthCodeParams {
            code_challenge: "",
            ..base
        })
        .await
        .expect_err("an empty challenge must not be silently dropped");
    ctx.repo
        .mint_authorization_code(MintAuthCodeParams {
            code_challenge_method: "plain",
            ..base
        })
        .await
        .expect_err("only S256 is accepted");

    let code = ctx
        .repo
        .mint_authorization_code(base)
        .await
        .expect("S256 mint");
    ctx.repo
        .validate_authorization_code(&code, &ctx.client_id, &ctx.redirect_uri, &ctx.pkce.verifier)
        .await
        .expect("minted code redeems with its verifier");
}

#[tokio::test]
async fn replayed_code_with_linked_refresh_token_revokes_the_family() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid", None).await;

    ctx.repo
        .validate_authorization_code(&code, &ctx.client_id, &ctx.redirect_uri, &ctx.pkce.verifier)
        .await
        .expect("first use");

    let rt = systemprompt_identifiers::RefreshTokenId::new(format!("rt-{}", Uuid::new_v4()));
    ctx.repo
        .store_refresh_token(systemprompt_oauth::repository::RefreshTokenParams {
            token_id: &rt,
            client_id: &ctx.client_id,
            user_id: &ctx.user_id,
            scope: "openid",
            expires_at: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
            family_id: None,
        })
        .await
        .expect("store refresh token");
    ctx.repo
        .link_auth_code_to_refresh_token(&code, &rt)
        .await
        .expect("link");

    let err = ctx
        .repo
        .validate_authorization_code(&code, &ctx.client_id, &ctx.redirect_uri, &ctx.pkce.verifier)
        .await
        .expect_err("replay must be rejected");
    assert!(err.to_string().contains("Invalid authorization code"));

    ctx.repo
        .consume_refresh_token(&rt, &ctx.client_id)
        .await
        .expect_err("family must be revoked after replay");
}

#[test]
fn auth_code_params_builder_sets_challenge_and_resource() {
    let code = AuthorizationCode::new("code-builder");
    let client = ClientId::new("client_builder");
    let user = UserId::new("user-builder");
    let params = AuthCodeParams::builder(
        &code,
        &client,
        &user,
        "http://127.0.0.1/cb",
        "openid",
        "challenge-value",
    )
    .with_resource("https://rs.example")
    .build();

    assert_eq!(params.code_challenge, "challenge-value");
    assert_eq!(params.resource, Some("https://rs.example"));
}

#[tokio::test]
async fn link_auth_code_to_dangling_refresh_token_errors() {
    let ctx = setup().await;
    let code = store_code(&ctx, "openid", None).await;

    assert!(
        ctx.repo
            .link_auth_code_to_refresh_token(&code, &RefreshTokenId::new("rt-id-value"))
            .await
            .is_err(),
        "refresh_token_id is a foreign key, so a dangling id is rejected"
    );
}
