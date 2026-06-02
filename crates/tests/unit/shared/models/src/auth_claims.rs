use std::collections::BTreeMap;

use systemprompt_models::auth::{
    ActClaim, JwtAudience, JwtClaims, MAX_ACT_CHAIN_DEPTH, Permission, RateLimitTier, TokenType,
    UserType,
};

fn claims(scope: Vec<Permission>, aud: Vec<JwtAudience>) -> JwtClaims {
    JwtClaims {
        sub: "user_1".to_owned(),
        iat: 1_000,
        exp: 5_000,
        nbf: None,
        iss: "test-issuer".to_owned(),
        aud,
        jti: "jti-1".to_owned(),
        scope,
        username: "alice".to_owned(),
        email: "alice@example.com".to_owned(),
        user_type: UserType::User,
        roles: vec!["editor".to_owned()],
        attributes: BTreeMap::new(),
        client_id: None,
        token_type: TokenType::Bearer,
        auth_time: 1_000,
        session_id: None,
        rate_limit_tier: Some(RateLimitTier::User),
        plugin_id: None,
        act: None,
    }
}

mod jwt_claims_methods {
    use super::*;

    #[test]
    fn has_permission_true_and_false() {
        let c = claims(vec![Permission::User, Permission::Mcp], vec![]);
        assert!(c.has_permission(Permission::User));
        assert!(c.has_permission(Permission::Mcp));
        assert!(!c.has_permission(Permission::Admin));
    }

    #[test]
    fn is_admin_user_anonymous_helpers() {
        assert!(claims(vec![Permission::Admin], vec![]).is_admin());
        assert!(!claims(vec![Permission::User], vec![]).is_admin());
        assert!(claims(vec![Permission::User], vec![]).is_registered_user());
        assert!(claims(vec![Permission::Anonymous], vec![]).is_anonymous());
        assert!(!claims(vec![Permission::User], vec![]).is_anonymous());
    }

    #[test]
    fn permissions_accessors_agree() {
        let c = claims(vec![Permission::User, Permission::A2a], vec![]);
        assert_eq!(c.permissions(), &[Permission::User, Permission::A2a]);
        assert_eq!(c.get_permissions(), vec![Permission::User, Permission::A2a]);
        let scopes = c.get_scopes();
        assert_eq!(scopes.len(), 2);
        assert!(scopes.contains(&"user".to_owned()));
    }

    #[test]
    fn has_audience_checks_membership() {
        let c = claims(
            vec![Permission::User],
            vec![JwtAudience::Api, JwtAudience::Mcp],
        );
        assert!(c.has_audience(&JwtAudience::Api));
        assert!(!c.has_audience(&JwtAudience::Web));
    }

    #[test]
    fn role_helpers() {
        let c = claims(vec![Permission::User], vec![]);
        assert!(c.has_role("editor"));
        assert!(!c.has_role("admin"));
        assert_eq!(c.roles(), &["editor".to_owned()]);
    }
}

mod jwt_claims_serde {
    use super::*;

    #[test]
    fn scope_serializes_as_space_string() {
        let c = claims(
            vec![Permission::User, Permission::Mcp],
            vec![JwtAudience::Api],
        );
        let value = serde_json::to_value(&c).expect("serialize");
        let scope = value["scope"].as_str().expect("scope is a string");
        assert!(scope.contains("user"));
        assert!(scope.contains("mcp"));
    }

    #[test]
    fn audiences_serialize_as_string_array() {
        let c = claims(
            vec![Permission::User],
            vec![JwtAudience::Api, JwtAudience::Hook],
        );
        let value = serde_json::to_value(&c).expect("serialize");
        let auds = value["aud"].as_array().expect("aud is an array");
        let strs: Vec<&str> = auds.iter().filter_map(|v| v.as_str()).collect();
        assert!(strs.contains(&"api"));
        assert!(strs.contains(&"hook"));
    }

    #[test]
    fn full_round_trip_preserves_fields() {
        let mut c = claims(
            vec![Permission::Admin, Permission::User],
            vec![JwtAudience::Api, JwtAudience::Resource("acme".to_owned())],
        );
        c.attributes
            .insert("acme.desk".to_owned(), serde_json::json!("fixed-income"));
        let json = serde_json::to_string(&c).expect("serialize");
        let back: JwtClaims = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.sub, c.sub);
        assert_eq!(back.scope, c.scope);
        assert_eq!(back.aud, c.aud);
        assert_eq!(back.roles, c.roles);
        assert_eq!(
            back.attributes.get("acme.desk"),
            Some(&serde_json::json!("fixed-income"))
        );
        assert_eq!(back.rate_limit_tier, Some(RateLimitTier::User));
    }

    #[test]
    fn resource_audience_round_trips() {
        let c = claims(
            vec![Permission::User],
            vec![JwtAudience::Resource("custom-aud".to_owned())],
        );
        let json = serde_json::to_string(&c).expect("serialize");
        let back: JwtClaims = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            back.aud,
            vec![JwtAudience::Resource("custom-aud".to_owned())]
        );
    }

    #[test]
    fn empty_roles_and_attributes_are_omitted() {
        let c = claims(vec![Permission::User], vec![JwtAudience::Api]);
        let mut bare = c;
        bare.roles.clear();
        let value = serde_json::to_value(&bare).expect("serialize");
        assert!(value.get("roles").is_none());
        assert!(value.get("attributes").is_none());
    }
}

mod act_claim_depth {
    use super::*;

    fn link(sub: &str, inner: Option<ActClaim>) -> ActClaim {
        ActClaim {
            iss: "iss".to_owned(),
            sub: sub.to_owned(),
            act: Box::new(inner),
        }
    }

    #[test]
    fn single_link_depth_is_one() {
        assert_eq!(link("a", None).depth(), 1);
    }

    #[test]
    fn three_link_depth_is_three() {
        let chain = link("c", Some(link("b", Some(link("a", None)))));
        assert_eq!(chain.depth(), 3);
    }

    #[test]
    fn depth_short_circuits_above_cap() {
        let mut current = link("leaf", None);
        for i in 0..(MAX_ACT_CHAIN_DEPTH + 5) {
            current = link(&format!("n{i}"), Some(current));
        }
        assert!(current.depth() > MAX_ACT_CHAIN_DEPTH);
    }

    #[test]
    fn flatten_truncates_at_cap() {
        let mut current = link("leaf", None);
        for i in 0..(MAX_ACT_CHAIN_DEPTH + 5) {
            current = link(&format!("n{i}"), Some(current));
        }
        let chain = current.flatten_to_chain();
        assert_eq!(chain.len(), MAX_ACT_CHAIN_DEPTH);
    }

    #[test]
    fn act_claim_round_trips_through_json() {
        let chain = link("outer", Some(link("inner", None)));
        let json = serde_json::to_string(&chain).expect("serialize");
        let back: ActClaim = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, chain);
        assert_eq!(back.depth(), 2);
    }
}
