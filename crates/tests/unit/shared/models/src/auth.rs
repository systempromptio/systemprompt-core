//! Unit tests for authentication models
//!
//! Tests cover:
//! - BaseRoles constants
//! - ActClaim delegation chains

use systemprompt_models::BaseRoles;

#[test]
fn test_base_roles_anonymous_constant() {
    assert_eq!(BaseRoles::ANONYMOUS, "anonymous");
}

#[test]
fn test_base_roles_user_constant() {
    assert_eq!(BaseRoles::USER, "user");
}

#[test]
fn test_base_roles_admin_constant() {
    assert_eq!(BaseRoles::ADMIN, "admin");
}

mod act_claim {
    use systemprompt_models::auth::ActClaim;

    fn nested(sub: &str, inner: Option<ActClaim>) -> ActClaim {
        ActClaim {
            iss: format!("iss-{sub}"),
            sub: sub.to_string(),
            act: Box::new(inner),
        }
    }

    #[test]
    fn serde_round_trip_preserves_chain() {
        let chain = nested("outer", Some(nested("middle", Some(nested("inner", None)))));
        let json = serde_json::to_string(&chain).expect("serialize");
        let parsed: ActClaim = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, chain);
    }

    #[test]
    fn act_field_omitted_when_none() {
        let leaf = nested("only", None);
        let json = serde_json::to_value(&leaf).expect("serialize");
        assert!(json.get("act").is_none(), "act should be skipped when None");
    }

    #[test]
    fn flatten_three_level_chain_returns_outermost_first() {
        let chain = nested("outer", Some(nested("middle", Some(nested("inner", None)))));
        let flat = chain.flatten_to_chain();
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].user_id.as_str(), "outer");
        assert_eq!(flat[1].user_id.as_str(), "middle");
        assert_eq!(flat[2].user_id.as_str(), "inner");
    }

    #[test]
    fn flatten_single_link_chain() {
        let leaf = nested("solo", None);
        let flat = leaf.flatten_to_chain();
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].user_id.as_str(), "solo");
    }
}

mod user_type_from_permissions {
    use systemprompt_models::auth::{Permission, UserType};

    #[test]
    fn admin_wins_over_lower_scopes() {
        let perms = [Permission::User, Permission::Admin, Permission::Service];
        assert_eq!(UserType::from_permissions(&perms), UserType::Admin);
    }

    #[test]
    fn precedence_is_privilege_descending() {
        assert_eq!(
            UserType::from_permissions(&[Permission::User]),
            UserType::User
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::A2a]),
            UserType::A2a
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::Mcp]),
            UserType::Mcp
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::Service]),
            UserType::Service
        );
    }

    #[test]
    fn hook_scopes_resolve_to_service_not_anon() {
        assert_eq!(
            UserType::from_permissions(&[Permission::HookGovern]),
            UserType::Service
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::HookTrack]),
            UserType::Service
        );
        assert_eq!(
            UserType::from_permissions(&[Permission::HookGovern, Permission::HookTrack]),
            UserType::Service
        );
    }

    #[test]
    fn empty_or_anonymous_only_is_anon() {
        assert_eq!(UserType::from_permissions(&[]), UserType::Anon);
        assert_eq!(
            UserType::from_permissions(&[Permission::Anonymous]),
            UserType::Anon
        );
    }
}
