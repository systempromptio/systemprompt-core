//! Regression coverage for JWT-claims → `AuthzRequest` forwarding on the MCP
//! middleware path. Locks in that `claims.roles` and `claims.attributes` are
//! passed verbatim into the hook input.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use systemprompt_identifiers::{Actor, ClientId, SessionId, UserId};
use systemprompt_mcp::middleware::rbac::build_mcp_authz_request;
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::authz::EntityRef;

fn claims_with(
    roles: Vec<String>,
    attributes: BTreeMap<String, serde_json::Value>,
) -> JwtClaims {
    let now = Utc::now();
    JwtClaims {
        sub: "user_42".to_string(),
        iat: now.timestamp(),
        exp: (now + Duration::hours(1)).timestamp(),
        nbf: Some(now.timestamp()),
        iss: "issuer".to_string(),
        aud: vec![JwtAudience::Mcp],
        jti: "jti-1".to_string(),
        scope: vec![Permission::User],
        username: "u".to_string(),
        email: "u@example.com".to_string(),
        user_type: UserType::User,
        roles,
        attributes,
        client_id: Some(ClientId::new("c")),
        token_type: TokenType::Bearer,
        auth_time: now.timestamp(),
        session_id: Some(SessionId::new("s")),
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    }
}

#[test]
fn forwards_roles_and_attributes_from_claims() {
    let mut attrs = BTreeMap::new();
    attrs.insert("acme.desk".to_owned(), serde_json::json!("fixed-income"));
    let claims = claims_with(
        vec!["eng".to_owned(), "platform".to_owned()],
        attrs.clone(),
    );
    let act_chain: Vec<Actor> = vec![Actor::user(UserId::new("user_42"))];

    let req = build_mcp_authz_request("server-x", &claims, act_chain.clone());

    assert_eq!(req.user_id.as_str(), "user_42");
    assert_eq!(req.roles, vec!["eng".to_owned(), "platform".to_owned()]);
    assert_eq!(req.attributes, attrs);
    assert_eq!(req.act_chain.len(), act_chain.len());
    assert!(matches!(req.entity, EntityRef::McpServer(_)));
}

#[test]
fn empty_attributes_round_trip() {
    let claims = claims_with(vec![], BTreeMap::new());
    let req = build_mcp_authz_request("server-x", &claims, Vec::new());
    assert!(req.attributes.is_empty());
}
