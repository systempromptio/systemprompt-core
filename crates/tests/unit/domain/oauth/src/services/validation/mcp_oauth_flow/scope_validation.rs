use systemprompt_models::auth::Permission;
use systemprompt_models::oauth::OAuthServerConfig;
use systemprompt_oauth::repository::OAuthRepository;

#[test]
fn test_valid_scopes_include_user_and_admin() {
    let available = OAuthRepository::get_available_scopes();
    let names: Vec<String> = available.iter().map(|(n, _)| n.clone()).collect();
    assert!(names.contains(&"user".to_string()));
    assert!(names.contains(&"admin".to_string()));
}

#[test]
fn test_validate_scopes_accepts_user() {
    let scopes = OAuthRepository::validate_scopes(&["user".to_string()])
        .expect("user scope should be valid");
    assert_eq!(scopes, vec!["user"]);
}

#[test]
fn test_validate_scopes_accepts_admin() {
    let scopes = OAuthRepository::validate_scopes(&["admin".to_string()])
        .expect("admin scope should be valid");
    assert_eq!(scopes, vec!["admin"]);
}

#[test]
fn test_validate_scopes_accepts_user_and_admin() {
    let scopes =
        OAuthRepository::validate_scopes(&["user".to_string(), "admin".to_string()])
            .expect("user and admin scopes should be valid");
    assert!(scopes.contains(&"user".to_string()));
    assert!(scopes.contains(&"admin".to_string()));
}

#[test]
fn test_validate_scopes_rejects_unknown() {
    OAuthRepository::validate_scopes(&["unknown_scope".to_string()]).unwrap_err();
}

#[test]
fn test_validate_scopes_rejects_openid() {
    OAuthRepository::validate_scopes(&["openid".to_string()]).unwrap_err();
}

#[test]
fn test_validate_scopes_empty_returns_empty() {
    let scopes = OAuthRepository::validate_scopes(&[])
        .expect("empty scopes should be valid");
    assert!(scopes.is_empty());
}

#[test]
fn test_parse_scopes_space_separated() {
    let scopes = OAuthRepository::parse_scopes("user admin");
    assert_eq!(scopes, vec!["user", "admin"]);
}

#[test]
fn test_parse_scopes_extra_whitespace() {
    let scopes = OAuthRepository::parse_scopes("  user   admin  ");
    assert_eq!(scopes, vec!["user", "admin"]);
}

#[test]
fn test_parse_scopes_single() {
    let scopes = OAuthRepository::parse_scopes("user");
    assert_eq!(scopes, vec!["user"]);
}

#[test]
fn test_parse_scopes_empty() {
    let scopes = OAuthRepository::parse_scopes("");
    assert!(scopes.is_empty());
}

#[test]
fn test_format_scopes_round_trips() {
    let scopes = vec!["user".to_string(), "admin".to_string()];
    let formatted = OAuthRepository::format_scopes(&scopes);
    assert_eq!(formatted, "user admin");
    let parsed = OAuthRepository::parse_scopes(&formatted);
    assert_eq!(parsed, scopes);
}

#[test]
fn test_default_roles_includes_user() {
    let defaults = OAuthRepository::get_default_roles();
    assert!(defaults.contains(&"user".to_string()));
}

#[test]
fn test_default_roles_does_not_include_admin() {
    let defaults = OAuthRepository::get_default_roles();
    assert!(!defaults.contains(&"admin".to_string()));
}

#[test]
fn test_available_scopes_superset_of_defaults() {
    let defaults = OAuthRepository::get_default_roles();
    let available: Vec<String> = OAuthRepository::get_available_scopes()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    for d in &defaults {
        assert!(available.contains(d), "Default scope {d} not in available scopes");
    }
}

#[test]
fn test_scope_exists_for_all_available() {
    for (name, _) in OAuthRepository::get_available_scopes() {
        assert!(
            OAuthRepository::scope_exists(&name),
            "Available scope {name} not found by scope_exists"
        );
    }
}

#[test]
fn test_permission_admin_implies_user() {
    assert!(Permission::Admin.implies(&Permission::User));
}

#[test]
fn test_permission_user_does_not_imply_admin() {
    assert!(!Permission::User.implies(&Permission::Admin));
}

#[test]
fn test_permission_admin_implies_all() {
    for perm in &[
        Permission::User,
        Permission::Anonymous,
        Permission::A2a,
        Permission::Mcp,
        Permission::Service,
    ] {
        assert!(
            Permission::Admin.implies(perm),
            "Admin should imply {:?}",
            perm
        );
    }
}

#[test]
fn test_permission_hierarchy_order() {
    assert!(Permission::Admin.hierarchy_level() > Permission::User.hierarchy_level());
    assert!(Permission::User.hierarchy_level() > Permission::Service.hierarchy_level());
    assert!(Permission::Service.hierarchy_level() > Permission::A2a.hierarchy_level());
    assert!(Permission::A2a.hierarchy_level() > Permission::Mcp.hierarchy_level());
    assert!(Permission::Mcp.hierarchy_level() > Permission::Anonymous.hierarchy_level());
}

#[test]
fn test_permission_from_str_all_variants() {
    use std::str::FromStr;
    for variant_str in Permission::ALL_VARIANTS {
        let result = Permission::from_str(variant_str);
        assert!(result.is_ok(), "Failed to parse '{variant_str}'");
    }
}

#[test]
fn test_permission_round_trip() {
    use std::str::FromStr;
    for variant_str in Permission::ALL_VARIANTS {
        let perm = Permission::from_str(variant_str).unwrap();
        assert_eq!(perm.as_str(), *variant_str);
    }
}

#[test]
fn test_wellknown_response_type_only_code() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert_eq!(config.supported_response_types, vec!["code"]);
}

#[test]
fn test_wellknown_code_challenge_methods_only_s256() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert_eq!(config.supported_code_challenge_methods, vec!["S256"]);
}

#[test]
fn test_wellknown_supported_scopes_match_valid_scopes() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    for scope in &config.supported_scopes {
        assert!(
            OAuthRepository::scope_exists(scope),
            "Well-known advertises scope '{scope}' that is not in VALID_SCOPES"
        );
    }
}

#[test]
fn test_wellknown_registration_endpoint_uses_register_path() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert!(
        config.registration_endpoint.ends_with("/register"),
        "Registration endpoint should end with /register, got: {}",
        config.registration_endpoint
    );
}

#[test]
fn test_wellknown_authorization_endpoint() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert_eq!(
        config.authorization_endpoint,
        "https://example.com/api/v1/core/oauth/authorize"
    );
}

#[test]
fn test_wellknown_token_endpoint() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert_eq!(
        config.token_endpoint,
        "https://example.com/api/v1/core/oauth/token"
    );
}

#[test]
fn test_wellknown_grant_types_include_authorization_code() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert!(config.supported_grant_types.contains(&"authorization_code".to_string()));
}

#[test]
fn test_wellknown_grant_types_include_refresh_token() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert!(config.supported_grant_types.contains(&"refresh_token".to_string()));
}

#[test]
fn test_wellknown_default_scope_is_user() {
    let config = OAuthServerConfig::from_api_server_url("https://example.com");
    assert_eq!(config.default_scope, "user");
}

#[test]
fn test_dynamic_registration_default_scopes() {
    let default_roles = OAuthRepository::get_default_roles();
    assert!(
        default_roles.contains(&"user".to_string()),
        "Dynamic registration defaults should include user"
    );
}

#[test]
fn test_scope_validation_consistent_with_permission_enum() {
    use std::str::FromStr;
    for (name, _) in OAuthRepository::get_available_scopes() {
        assert!(
            Permission::from_str(&name).is_ok(),
            "Valid scope '{name}' cannot be parsed as Permission"
        );
    }
}

#[test]
fn test_mcp_server_scope_user_admin_both_valid() {
    let mcp_scopes = vec!["user".to_string(), "admin".to_string()];
    OAuthRepository::validate_scopes(&mcp_scopes)
        .expect("user and admin MCP scopes should be valid");
}

#[test]
fn test_client_scope_check_bypass_when_resource_covers() {
    let resource_scopes = "user admin";
    let requested_scopes = vec!["user".to_string(), "admin".to_string()];
    let resource_scope_list = OAuthRepository::parse_scopes(resource_scopes);
    let all_covered = requested_scopes
        .iter()
        .all(|s| resource_scope_list.contains(s));
    assert!(all_covered);
}

#[test]
fn test_client_scope_check_no_bypass_when_resource_partial() {
    let resource_scopes = "user";
    let requested_scopes = vec!["user".to_string(), "admin".to_string()];
    let resource_scope_list = OAuthRepository::parse_scopes(resource_scopes);
    let all_covered = requested_scopes
        .iter()
        .all(|s| resource_scope_list.contains(s));
    assert!(!all_covered);
}
