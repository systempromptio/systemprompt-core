//! OAuth PKCE fixture smoke tests.
//!
//! Exercises `seed_oauth_client` and `pkce_pair` end-to-end: seeds a
//! confidential client, verifies it round-trips through `ClientRepository`,
//! and checks the PKCE pair has the expected S256 shape (43–128 char
//! verifier, base64url-no-pad challenge).
//!
//! A full HTTP authorize → /token round trip is intentionally out of scope
//! here — that drives the OAuth `core::public_router`, which needs an
//! `OAuthState` and consent flow that aren't trivially constructible from the
//! AppContext fixture. The fixture+repository smoke ensures future work can
//! build on a known-good client seed.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use systemprompt_identifiers::UserId;
use systemprompt_oauth::repository::ClientRepository;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_db_pool, pkce_pair, seed_oauth_client,
};
use uuid::Uuid;

async fn seed_user(pool: &systemprompt_database::DbPool, user_id: &UserId) {
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user_id.as_str())
        .bind(format!("{}@oauth-fixture.invalid", user_id.as_str()))
        .execute(p.as_ref())
        .await
        .expect("seed user");
}

#[tokio::test]
async fn seed_oauth_client_inserts_and_finds_by_id() -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user_id = UserId::new(format!("oauth-owner-{}", Uuid::new_v4()));
    seed_user(&pool, &user_id).await;

    let OAuthClientFixture {
        client_id,
        redirect_uri,
        ..
    } = seed_oauth_client(&pool, &user_id).await?;

    let repo = ClientRepository::new(&pool).expect("client repo");
    let found = repo
        .get_by_client_id(&client_id)
        .await
        .expect("get_by_client_id")
        .expect("client present");
    assert_eq!(found.client_id, client_id);
    assert!(found.redirect_uris.iter().any(|u| u == &redirect_uri));
    Ok(())
}

#[tokio::test]
async fn pkce_pair_has_s256_shape() {
    let pair = pkce_pair();
    assert_eq!(pair.method, "S256");
    assert!(
        pair.verifier.len() >= 43 && pair.verifier.len() <= 128,
        "verifier length {} outside RFC 7636 bounds",
        pair.verifier.len()
    );
    // Re-derive the challenge and confirm it matches: sha256 → base64url-no-pad.
    let mut hasher = Sha256::new();
    hasher.update(pair.verifier.as_bytes());
    let expected = URL_SAFE_NO_PAD.encode(hasher.finalize());
    assert_eq!(pair.challenge, expected);
}

#[tokio::test]
async fn pkce_pair_is_random_per_call() {
    let a = pkce_pair();
    let b = pkce_pair();
    assert_ne!(a.verifier, b.verifier);
    assert_ne!(a.challenge, b.challenge);
}
