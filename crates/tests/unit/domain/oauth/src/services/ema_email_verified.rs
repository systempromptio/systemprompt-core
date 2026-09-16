use systemprompt_oauth::services::ema::EnterprisePrincipal;
use systemprompt_oauth::services::validation::id_jag::IdJagClaims;

fn principal(email_verified: bool) -> EnterprisePrincipal {
    EnterprisePrincipal {
        issuer: "https://idp.example".to_owned(),
        sub: "user-1".to_owned(),
        email: Some("admin@example.com".to_owned()),
        email_verified,
    }
}

#[test]
fn an_email_claim_alone_is_not_a_verified_email() {
    let claims = principal(false).verified_claims();
    assert_eq!(claims.email.as_deref(), Some("admin@example.com"));
    assert!(
        !claims.email_verified,
        "a bare email claim used to be treated as verified and linked to an existing account"
    );
}

#[test]
fn the_issuer_assertion_is_carried_verbatim() {
    assert!(principal(true).verified_claims().email_verified);
}

#[test]
fn a_verified_flag_without_an_email_verifies_nothing() {
    let claims = EnterprisePrincipal {
        email: None,
        ..principal(true)
    }
    .verified_claims();
    assert!(!claims.email_verified);
}

#[test]
fn id_jag_claims_default_email_verified_to_false() {
    let claims: IdJagClaims = serde_json::from_value(serde_json::json!({
        "iss": "https://idp.example", "sub": "user-1", "aud": "https://core.example",
        "jti": "jti-1", "exp": 1_000_000, "iat": 999_700, "email": "admin@example.com"
    }))
    .expect("claims without email_verified still decode");
    assert!(!claims.email_verified);
}
