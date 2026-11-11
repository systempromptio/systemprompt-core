//! The RS256 decode primitive verifies against the public key set alone.
//!
//! A verifier built from the serialised [`TokenVerificationKeys`] (public
//! material only, as a subprocess receives it) accepts tokens signed by the
//! active and the staged key, so promoting the staged key needs no verifier
//! restart, and refuses a key outside the set or a retired key past expiry.

use chrono::{Duration, Utc};
use systemprompt_security::AuthError;
use systemprompt_security::keys::{RetiredJwk, TokenVerificationKeys, VerificationKeyring};

use crate::signing_key_rotation::{mint, verify_with};

fn public_only(keys: &TokenVerificationKeys) -> VerificationKeyring {
    let json = serde_json::to_string(keys).expect("serialise");
    assert!(
        !json.contains("PRIVATE"),
        "the key set carries no private material"
    );
    VerificationKeyring::new(serde_json::from_str(&json).expect("parse")).expect("keyring")
}

#[test]
fn a_token_signed_by_the_staged_key_verifies_before_promotion() {
    let active = systemprompt_test_fixtures::next_test_key();
    let staged = systemprompt_test_fixtures::next_test_key();
    let keyring = public_only(&TokenVerificationKeys {
        active: active.jwk(),
        staged: Some(staged.jwk()),
        retired: Vec::new(),
    });

    verify_with(&keyring, &mint(&staged)).expect("staged key verifies");
    verify_with(&keyring, &mint(&active)).expect("active key verifies");
}

#[test]
fn without_the_staged_key_in_the_set_its_token_is_unknown() {
    let active = systemprompt_test_fixtures::next_test_key();
    let staged = systemprompt_test_fixtures::next_test_key();
    let keyring = public_only(&TokenVerificationKeys {
        active: active.jwk(),
        staged: None,
        retired: Vec::new(),
    });

    let err = verify_with(&keyring, &mint(&staged)).expect_err("outside the set");
    assert!(matches!(err, AuthError::UnknownKid(_)), "{err:?}");
}

#[test]
fn a_retired_key_verifies_until_its_expiry_and_not_after() {
    let active = systemprompt_test_fixtures::next_test_key();
    let retired = systemprompt_test_fixtures::next_test_key();
    let token = mint(&retired);
    let set = |expires_at| TokenVerificationKeys {
        active: active.jwk(),
        staged: None,
        retired: vec![RetiredJwk {
            jwk: retired.jwk(),
            expires_at,
        }],
    };

    verify_with(&public_only(&set(Utc::now() + Duration::hours(1))), &token)
        .expect("live retired key verifies");
    let err = verify_with(
        &public_only(&set(Utc::now() - Duration::seconds(1))),
        &token,
    )
    .expect_err("expired retired key");
    assert!(matches!(err, AuthError::UnknownKid(_)), "{err:?}");
}
