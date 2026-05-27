//! Test-only signing key install and bridge JWT minting helpers.
//!
//! [`install_test_signing_key`] is idempotent — the underlying authority cell
//! is process-wide, so the first caller wins. Concurrent test runs must share
//! the same minted key, which is fine because every test that consumes a JWT
//! resolves it via [`mint_admin_jwt`].

use std::sync::OnceLock;

use chrono::Duration;
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_security::jwt::{AdminTokenParams, JwtService};
use systemprompt_security::keys::RsaSigningKey;
use systemprompt_security::keys::authority::install_for_test;

static SIGNING_KEY: OnceLock<RsaSigningKey> = OnceLock::new();

pub fn install_test_signing_key() -> &'static RsaSigningKey {
    SIGNING_KEY.get_or_init(|| {
        let key = RsaSigningKey::generate_bits(2048).expect("rsa keygen");
        install_for_test(key.clone());
        key
    })
}

pub fn mint_admin_jwt(user_id: &UserId, email: &str, issuer: &str) -> JwtToken {
    install_test_signing_key();
    let session = SessionId::generate();
    let params = AdminTokenParams {
        user_id,
        session_id: &session,
        email,
        issuer,
        duration: Duration::hours(1),
        client_id: None,
    };
    JwtService::generate_admin_token(&params).expect("mint admin jwt")
}
