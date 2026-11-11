//! The proxy-to-MCP-server hop token and the owner-hop marker: what verifies
//! and everything that must not.

use chrono::{Duration, Utc};
use systemprompt_identifiers::{McpServerId, ServiceName, SessionId, UserId};
use systemprompt_models::auth::{
    AuthenticatedUser, McpHopIdentity, Permission, TokenAuthority, TokenError, UserType,
};
use systemprompt_security::jwt::{
    encode_mcp_hop, mcp_hop_claims, mint_owner_hop, verify_owner_hop,
};
use systemprompt_security::keys::{RsaSigningKey, SigningKeys};
use systemprompt_security::{AuthError, DeploymentTokenAuthority};
use systemprompt_test_fixtures::test_signing_keys;

const ISSUER: &str = "test-issuer";

fn server() -> McpServerId {
    McpServerId::new("documents")
}

fn identity() -> McpHopIdentity {
    McpHopIdentity {
        user_id: UserId::new("user-hop"),
        user: Some(AuthenticatedUser::new_with_roles(
            UserId::new("user-hop"),
            "hop".to_owned(),
            "hop@test.com".to_owned(),
            vec![Permission::User],
            vec!["editor".to_owned()],
        )),
        session_id: SessionId::new("sess-hop"),
        user_type: UserType::User,
        client_id: None,
        act_chain: Vec::new(),
        purpose: None,
        interaction: None,
        mcp_session: Some("mcp-sess-1".to_owned()),
    }
}

fn authority() -> DeploymentTokenAuthority {
    DeploymentTokenAuthority::new(test_signing_keys(), ISSUER)
}

#[test]
fn a_minted_hop_token_verifies_for_its_server_with_its_identity() {
    let authority = authority();
    let token = authority
        .mint_mcp_hop(&server(), &identity())
        .expect("mint");
    let claims = authority.verify_mcp_hop(&token, &server()).expect("verify");
    assert_eq!(claims.identity.user_id.as_str(), "user-hop");
    assert_eq!(claims.identity.mcp_session.as_deref(), Some("mcp-sess-1"));
    let user = claims.identity.user.expect("user");
    assert_eq!(user.roles, vec!["editor".to_owned()]);
    assert!(claims.registered.exp - claims.registered.iat <= 60);
}

#[test]
fn a_hop_token_for_another_server_is_refused() {
    let authority = authority();
    let token = authority
        .mint_mcp_hop(&McpServerId::new("other"), &identity())
        .expect("mint");
    let err = authority
        .verify_mcp_hop(&token, &server())
        .expect_err("audience");
    assert!(matches!(err, TokenError::InvalidAudience), "{err:?}");
}

#[test]
fn an_expired_hop_token_is_refused() {
    let keys = test_signing_keys();
    let mut claims = mcp_hop_claims(ISSUER, &server(), &identity());
    let past = (Utc::now() - Duration::minutes(2)).timestamp();
    claims.registered.iat = past - 30;
    claims.registered.nbf = Some(past - 30);
    claims.registered.exp = past;
    let token = encode_mcp_hop(&keys, &claims).expect("encode");
    let err = authority()
        .verify_mcp_hop(&token, &server())
        .expect_err("expired");
    assert!(matches!(err, TokenError::Expired), "{err:?}");
}

#[test]
fn a_hop_token_signed_by_an_unknown_key_is_refused() {
    let foreign =
        SigningKeys::new(RsaSigningKey::generate().expect("key"), &[9; 32]).expect("keys");
    let token =
        encode_mcp_hop(&foreign, &mcp_hop_claims(ISSUER, &server(), &identity())).expect("encode");
    let err = authority()
        .verify_mcp_hop(&token, &server())
        .expect_err("kid");
    assert!(matches!(err, TokenError::UnknownKid(_)), "{err:?}");
}

#[test]
fn a_bearer_signed_by_the_deployment_key_is_not_a_hop_token() {
    let authority = authority();
    let claims = mcp_hop_claims(ISSUER, &server(), &identity());
    let value = serde_json::to_value(&claims).expect("json");
    let token = authority.sign_claims(&value).expect("sign");
    let err = authority
        .verify_mcp_hop(&token, &server())
        .expect_err("typ");
    let TokenError::Rejected(source) = err else {
        panic!("expected Rejected, got {err:?}");
    };
    assert!(matches!(
        source.downcast_ref::<AuthError>(),
        Some(AuthError::UnexpectedTokenType { .. })
    ));
}

#[test]
fn a_long_lived_hop_token_is_refused() {
    let keys = test_signing_keys();
    let mut claims = mcp_hop_claims(ISSUER, &server(), &identity());
    claims.registered.exp = claims.registered.iat + Duration::hours(1).num_seconds();
    let token = encode_mcp_hop(&keys, &claims).expect("encode");
    let err = authority()
        .verify_mcp_hop(&token, &server())
        .expect_err("lifetime");
    assert!(matches!(err, TokenError::Rejected(_)), "{err:?}");
}

#[test]
fn a_hop_token_whose_subject_differs_from_its_identity_is_refused() {
    let keys = test_signing_keys();
    let mut claims = mcp_hop_claims(ISSUER, &server(), &identity());
    claims.registered.sub = "someone-else".to_owned();
    let token = encode_mcp_hop(&keys, &claims).expect("encode");
    let err = authority()
        .verify_mcp_hop(&token, &server())
        .expect_err("sub");
    assert!(matches!(err, TokenError::Rejected(_)), "{err:?}");
}

#[test]
fn an_owner_hop_marker_verifies_only_for_its_service() {
    let keys = test_signing_keys();
    let service = ServiceName::new("documents");
    let marker = mint_owner_hop(&keys, ISSUER, &service).expect("mint");
    verify_owner_hop(keys.keyring(), ISSUER, &service, &marker).expect("verify");
    verify_owner_hop(keys.keyring(), ISSUER, &ServiceName::new("other"), &marker)
        .expect_err("another service");
    verify_owner_hop(keys.keyring(), ISSUER, &service, "1").expect_err("client-supplied");
}

#[test]
fn a_hop_token_is_not_an_owner_hop_marker() {
    let keys = test_signing_keys();
    let token = authority()
        .mint_mcp_hop(&server(), &identity())
        .expect("mint");
    let err = verify_owner_hop(
        keys.keyring(),
        ISSUER,
        &ServiceName::new("documents"),
        &token,
    )
    .expect_err("typ");
    assert!(
        matches!(err, AuthError::UnexpectedTokenType { .. }),
        "{err:?}"
    );
}
