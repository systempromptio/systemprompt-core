use std::str::FromStr;

use systemprompt_models::auth::{
    JwtAudience, Permission, RateLimitTier, TokenType, UserRole, UserStatus, UserType,
};

#[test]
fn jwt_audience_as_str_matches_known_variants() {
    assert_eq!(JwtAudience::Web.as_str(), "web");
    assert_eq!(JwtAudience::Api.as_str(), "api");
    assert_eq!(JwtAudience::A2a.as_str(), "a2a");
    assert_eq!(JwtAudience::Mcp.as_str(), "mcp");
    assert_eq!(JwtAudience::Internal.as_str(), "internal");
    assert_eq!(JwtAudience::Bridge.as_str(), "bridge");
    assert_eq!(JwtAudience::Hook.as_str(), "hook");
    assert_eq!(
        JwtAudience::Resource("custom".to_owned()).as_str(),
        "custom"
    );
}

#[test]
fn jwt_audience_display_matches_as_str() {
    assert_eq!(JwtAudience::Api.to_string(), "api");
    assert_eq!(JwtAudience::Resource("x".to_owned()).to_string(), "x");
}

#[test]
fn jwt_audience_from_str_round_trips_known_and_resource() {
    for s in ["web", "api", "a2a", "mcp", "internal", "bridge", "hook"] {
        let parsed = JwtAudience::from_str(s).unwrap();
        assert_eq!(parsed.as_str(), s);
    }
    let parsed = JwtAudience::from_str("custom-aud").unwrap();
    assert!(matches!(parsed, JwtAudience::Resource(ref s) if s == "custom-aud"));
}

#[test]
fn jwt_audience_standard_and_service_sets() {
    let standard = JwtAudience::standard();
    assert_eq!(standard.len(), 4);
    assert!(standard.contains(&JwtAudience::Web));
    assert!(standard.contains(&JwtAudience::Api));

    let service = JwtAudience::service();
    assert!(service.contains(&JwtAudience::Internal));
    assert!(!service.contains(&JwtAudience::Web));
}

#[test]
fn user_type_from_permissions_precedence_admin_first() {
    let perms = [Permission::Admin, Permission::User, Permission::A2a];
    assert_eq!(UserType::from_permissions(&perms), UserType::Admin);
    let perms = [Permission::User, Permission::A2a];
    assert_eq!(UserType::from_permissions(&perms), UserType::User);
    let perms = [Permission::A2a, Permission::Mcp];
    assert_eq!(UserType::from_permissions(&perms), UserType::A2a);
    let perms = [Permission::Mcp];
    assert_eq!(UserType::from_permissions(&perms), UserType::Mcp);
    let perms = [Permission::Service];
    assert_eq!(UserType::from_permissions(&perms), UserType::Service);
    let perms = [Permission::HookGovern];
    assert_eq!(UserType::from_permissions(&perms), UserType::Service);
    let perms = [Permission::HookTrack];
    assert_eq!(UserType::from_permissions(&perms), UserType::Service);
    let perms: [Permission; 0] = [];
    assert_eq!(UserType::from_permissions(&perms), UserType::Anon);
}

#[test]
fn user_type_as_str_and_display() {
    for v in [
        UserType::Admin,
        UserType::User,
        UserType::A2a,
        UserType::Mcp,
        UserType::Service,
        UserType::Anon,
        UserType::Unknown,
    ] {
        assert_eq!(v.to_string(), v.as_str());
    }
}

#[test]
fn user_type_from_str_known_variants() {
    for s in ["admin", "user", "a2a", "mcp", "service", "anon", "unknown"] {
        let parsed = UserType::from_str(s).unwrap();
        assert_eq!(parsed.as_str(), s);
    }
    assert!(UserType::from_str("nope").is_err());
}

#[test]
fn user_type_rate_tier_maps() {
    assert_eq!(UserType::Admin.rate_tier(), RateLimitTier::Admin);
    assert_eq!(UserType::User.rate_tier(), RateLimitTier::User);
    assert_eq!(UserType::A2a.rate_tier(), RateLimitTier::A2a);
    assert_eq!(UserType::Mcp.rate_tier(), RateLimitTier::Mcp);
    assert_eq!(UserType::Service.rate_tier(), RateLimitTier::Service);
    assert_eq!(UserType::Anon.rate_tier(), RateLimitTier::Anon);
    assert_eq!(UserType::Unknown.rate_tier(), RateLimitTier::Anon);
}

#[test]
fn user_type_reconcile_downgrades_admin_when_user_not_admin() {
    assert_eq!(UserType::Admin.reconcile_with(false), UserType::User);
    assert_eq!(UserType::Admin.reconcile_with(true), UserType::Admin);
    assert_eq!(UserType::Service.reconcile_with(false), UserType::Service);
}

#[test]
fn token_type_default_is_bearer() {
    assert_eq!(TokenType::default(), TokenType::Bearer);
    assert_eq!(TokenType::Bearer.as_str(), "Bearer");
    assert_eq!(TokenType::Bearer.to_string(), "Bearer");
}

#[test]
fn rate_limit_tier_round_trips_all_variants() {
    for v in [
        RateLimitTier::Admin,
        RateLimitTier::User,
        RateLimitTier::A2a,
        RateLimitTier::Mcp,
        RateLimitTier::Service,
        RateLimitTier::Anon,
    ] {
        let s = v.as_str();
        assert_eq!(RateLimitTier::from_str(s).unwrap(), v);
        assert_eq!(v.to_string(), s);
    }
    assert!(RateLimitTier::from_str("nope").is_err());
}

#[test]
fn user_role_round_trips_all_variants() {
    for v in [UserRole::Admin, UserRole::User, UserRole::Anonymous] {
        let s = v.as_str();
        assert_eq!(UserRole::from_str(s).unwrap(), v);
        assert_eq!(v.to_string(), s);
    }
    assert!(UserRole::from_str("nope").is_err());
}

#[test]
fn user_status_round_trips_all_variants_and_is_active() {
    for v in [
        UserStatus::Active,
        UserStatus::Inactive,
        UserStatus::Suspended,
        UserStatus::Pending,
        UserStatus::Deleted,
        UserStatus::Temporary,
    ] {
        let s = v.as_str();
        assert_eq!(UserStatus::from_str(s).unwrap(), v);
        assert_eq!(v.to_string(), s);
    }
    assert!(UserStatus::Active.is_active());
    assert!(!UserStatus::Inactive.is_active());
    assert!(!UserStatus::Suspended.is_active());
    assert!(UserStatus::from_str("bogus").is_err());
}

#[test]
fn user_type_serde_lowercase() {
    let json = serde_json::to_string(&UserType::Admin).unwrap();
    assert_eq!(json, "\"admin\"");
}

#[test]
fn jwt_audience_serde_resource_is_untagged_string() {
    let aud = JwtAudience::Resource("vault-server".to_owned());
    let json = serde_json::to_string(&aud).unwrap();
    assert_eq!(json, "\"vault-server\"");
}
