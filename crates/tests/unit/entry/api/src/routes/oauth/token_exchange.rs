//! Pure-logic coverage for the RFC 8693 token-exchange surface.
//!
//! End-to-end exchange (against a real DB, OAuthState, and trusted
//! issuer's JWKS) is exercised by the OAuth integration tests; these
//! cover the algorithmic primitives that the endpoint is built on:
//! scope intersection, `act` chain construction, and the unsafe issuer
//! peek used to route a subject token to the right verification path.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use systemprompt_api::routes::oauth::endpoints::token::generation::{
    build_act_chain, intersect_scopes, peek_issuer,
};
use systemprompt_identifiers::ClientId;
use systemprompt_models::auth::{ActClaim, Permission};

fn jwt_from_payload(payload: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"RS256\",\"kid\":\"k1\"}");
    let body = URL_SAFE_NO_PAD.encode(payload);
    format!("{header}.{body}.sig")
}

#[test]
fn peek_issuer_recovers_issuer_from_unsigned_jwt() {
    let token = jwt_from_payload(r#"{"iss":"https://idp.example.com","sub":"u-1"}"#);
    let iss = peek_issuer(&token).expect("peek_issuer should succeed");
    assert_eq!(iss, "https://idp.example.com");
}

#[test]
fn peek_issuer_rejects_non_jwt() {
    assert!(peek_issuer("not-a-jwt").is_err());
}

#[test]
fn intersect_scopes_keeps_only_overlap_across_subject_client_owner() {
    let requested = vec![Permission::Admin, Permission::User];
    let subject = vec![Permission::Admin, Permission::User];
    let client = vec![Permission::User];
    let owner = vec![Permission::Admin, Permission::User];
    let out = intersect_scopes(&requested, &subject, &client, &owner).expect("non-empty");
    assert_eq!(out, vec![Permission::User]);
}

#[test]
fn intersect_scopes_empty_overlap_errors() {
    let requested = vec![Permission::Admin];
    let subject = vec![Permission::User];
    let client = vec![Permission::User];
    let owner = vec![Permission::User];
    assert!(intersect_scopes(&requested, &subject, &client, &owner).is_err());
}

#[test]
fn build_act_chain_with_no_prior_yields_single_link() {
    let cid = ClientId::new("client-1");
    let chain = build_act_chain(&cid, "https://core.example.com", None);
    assert_eq!(chain.iss, "https://core.example.com");
    assert_eq!(chain.sub, "client-1");
    assert!(chain.act.is_none());
}

#[test]
fn build_act_chain_extends_an_existing_chain() {
    let cid = ClientId::new("client-2");
    let prior = ActClaim {
        iss: "https://idp.example.com".to_string(),
        sub: "client-1".to_string(),
        act: Box::new(None),
    };
    let chain = build_act_chain(&cid, "https://core.example.com", Some(prior));
    assert_eq!(chain.sub, "client-2");
    let inner = chain.act.as_ref().as_ref().expect("inner act present");
    assert_eq!(inner.sub, "client-1");
    assert!(inner.act.is_none());
}

#[test]
fn build_act_chain_round_trip_through_flatten() {
    let cid = ClientId::new("client-3");
    let inner = ActClaim {
        iss: "https://idp.example.com".to_string(),
        sub: "client-1".to_string(),
        act: Box::new(None),
    };
    let middle = ActClaim {
        iss: "https://relay.example.com".to_string(),
        sub: "client-2".to_string(),
        act: Box::new(Some(inner)),
    };
    let chain = build_act_chain(&cid, "https://core.example.com", Some(middle));
    let flat = chain.flatten_to_chain();
    assert_eq!(flat.len(), 3);
    assert_eq!(flat[0].user_id.as_str(), "client-3");
    assert_eq!(flat[1].user_id.as_str(), "client-2");
    assert_eq!(flat[2].user_id.as_str(), "client-1");
}
