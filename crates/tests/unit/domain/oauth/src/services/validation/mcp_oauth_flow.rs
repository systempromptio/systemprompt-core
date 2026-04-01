use std::str::FromStr;
use systemprompt_models::auth::Permission;
use systemprompt_models::oauth::OAuthServerConfig;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::DynamicRegistrationRequest;

#[test]
fn test_valid_scopes_include_user_and_admin() {
    let available = OAuthRepository::get_available_scopes();
    let names: Vec<String> = available.iter().map(|(n, _)| n.clone()).collect();
    assert!(names.contains(&"user".to_string()));
    assert!(names.contains(&"admin".to_string()));
}

#[test]
fn test_validate_scopes_accepts_user() {
    let result = OAuthRepository::validate_scopes(&["user".to_string()]);
    let val = result.expect("expected success");
    assert_eq!(val, vec!["user"]);
}

#[test]
fn test_validate_scopes_accepts_admin() {
    let result = OAuthRepository::validate_scopes(&["admin".to_string()]);
    let val = result.expect("expected success");
    assert_eq!(val, vec!["admin"]);
}

#[test]
fn test_validate_scopes_accepts_user_and_admin() {
    let result =
        OAuthRepository::validate_scopes(&["user".to_string(), "admin".to_string()]);
    let scopes = result.expect("expected success");
    assert!(scopes.contains(&"user".to_string()));
    assert!(scopes.contains(&"admin".to_string()));
}

#[test]
fn test_validate_scopes_rejects_unknown() {
    let result = OAuthRepository::validate_scopes(&["unknown_scope".to_string()]);
    result.unwrap_err();
}

#[test]
fn test_validate_scopes_rejects_openid() {
    let result = OAuthRepository::validate_scopes(&["openid".to_string()]);
    result.unwrap_err();
}

#[test]
fn test_validate_scopes_empty_returns_empty() {
    let result = OAuthRepository::validate_scopes(&[]);
    let val = result.expect("expected success");
    assert!(val.is_empty());
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
    let result = OAuthRepository::validate_scopes(&mcp_scopes);
    result.expect("expected success");
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

#[test]
fn test_pkce_constants_valid() {
    use systemprompt_oauth::constants::pkce;
    assert!(pkce::CODE_CHALLENGE_MIN_LENGTH >= 43);
    assert!(pkce::CODE_CHALLENGE_MAX_LENGTH >= pkce::CODE_CHALLENGE_MIN_LENGTH);
    assert!(pkce::CODE_CHALLENGE_MAX_LENGTH <= 128);
}

#[test]
fn test_auth_code_expiry_reasonable() {
    let config = OAuthServerConfig::default();
    assert!(config.auth_code_expiry_seconds > 0);
    assert!(config.auth_code_expiry_seconds <= 600);
}

#[test]
fn test_access_token_expiry_reasonable() {
    let config = OAuthServerConfig::default();
    assert!(config.access_token_expiry_seconds > 0);
    assert!(config.access_token_expiry_seconds <= 86400);
}

// ============================================================================
// MCP Client Scenario Tests
//
// These tests simulate the logical flow for different MCP client behaviors
// to ensure all paths produce correct results.
// ============================================================================

fn simulate_determine_scopes(request: &DynamicRegistrationRequest) -> Vec<String> {
    if let Some(scope_string) = &request.scope {
        let requested_scopes: Vec<String> = scope_string
            .split_whitespace()
            .map(ToString::to_string)
            .collect();
        if !requested_scopes.is_empty() {
            if let Ok(valid) = OAuthRepository::validate_scopes(&requested_scopes) {
                return valid;
            }
        }
    }
    let default_roles = OAuthRepository::get_default_roles();
    if default_roles.is_empty() {
        vec!["user".to_string()]
    } else {
        default_roles
    }
}

fn simulate_authorize_scope_check(
    _client_scopes: &[String],
    requested_scopes: &[String],
    _resource_scopes: Option<&str>,
) -> Result<(), String> {
    OAuthRepository::validate_scopes(requested_scopes)
        .map_err(|e| format!("Invalid scopes: {e}"))?;
    Ok(())
}

fn simulate_token_permission_resolution(
    requested_scopes: &[String],
    user_permissions: &[Permission],
    _client_scopes: &[String],
    _resource_scopes: Option<&[String]>,
) -> Vec<Permission> {
    let mut final_permissions = Vec::new();
    for requested in requested_scopes {
        if let Ok(perm) = Permission::from_str(requested) {
            if perm == Permission::User {
                final_permissions.extend(
                    user_permissions.iter().filter(|p| p.is_user_role()).copied(),
                );
            } else if user_permissions.contains(&perm) {
                final_permissions.push(perm);
            }
        }
    }
    final_permissions.sort_by_key(|p| std::cmp::Reverse(p.hierarchy_level()));
    final_permissions.dedup();
    final_permissions
}

#[test]
fn scenario_claude_code_no_scope_in_registration() {
    let request: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Claude Code",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let scopes = simulate_determine_scopes(&request);
    assert!(scopes.contains(&"user".to_string()));
}

#[test]
fn scenario_mcp_inspector_with_scope_in_registration() {
    let request: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "MCP Inspector",
        "redirect_uris": ["http://localhost:5173/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "scope": "user admin",
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let scopes = simulate_determine_scopes(&request);
    assert!(scopes.contains(&"user".to_string()));
    assert!(scopes.contains(&"admin".to_string()));
}

#[test]
fn scenario_client_requests_only_user_scope() {
    let request: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Basic Client",
        "redirect_uris": ["https://example.com/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "scope": "user",
        "token_endpoint_auth_method": "client_secret_post"
    }"#).unwrap();

    let scopes = simulate_determine_scopes(&request);
    assert_eq!(scopes, vec!["user"]);
}

#[test]
fn scenario_authorize_with_resource_bypasses_client_scope_check() {
    let client_scopes = vec!["user".to_string()];
    let requested_scopes = vec!["user".to_string(), "admin".to_string()];
    let resource_scopes = Some("user admin");

    let result =
        simulate_authorize_scope_check(&client_scopes, &requested_scopes, resource_scopes);
    result.expect("expected success");
}

#[test]
fn scenario_authorize_without_resource_no_client_scope_check() {
    let client_scopes = vec!["user".to_string()];
    let requested_scopes = vec!["user".to_string(), "admin".to_string()];

    let result = simulate_authorize_scope_check(&client_scopes, &requested_scopes, None);
    assert!(
        result.is_ok(),
        "Authorization no longer checks client scopes — user permissions are the security boundary"
    );
}

#[test]
fn scenario_authorize_broad_client_scopes_no_resource_needed() {
    let client_scopes = vec!["user".to_string(), "admin".to_string()];
    let requested_scopes = vec!["user".to_string(), "admin".to_string()];

    let result = simulate_authorize_scope_check(&client_scopes, &requested_scopes, None);
    result.expect("expected success");
}

#[test]
fn scenario_authorize_subset_of_client_scopes() {
    let client_scopes = vec!["user".to_string(), "admin".to_string()];
    let requested_scopes = vec!["user".to_string()];

    let result = simulate_authorize_scope_check(&client_scopes, &requested_scopes, None);
    result.expect("expected success");
}

#[test]
fn scenario_token_user_gets_user_scope_only() {
    let requested = vec!["user".to_string(), "admin".to_string()];
    let user_perms = vec![Permission::User];
    let client_scopes = vec!["user".to_string(), "admin".to_string()];

    let result =
        simulate_token_permission_resolution(&requested, &user_perms, &client_scopes, None);
    assert_eq!(result, vec![Permission::User]);
}

#[test]
fn scenario_token_admin_gets_both_scopes() {
    let requested = vec!["user".to_string(), "admin".to_string()];
    let user_perms = vec![Permission::Admin, Permission::User];
    let client_scopes = vec!["user".to_string(), "admin".to_string()];

    let result =
        simulate_token_permission_resolution(&requested, &user_perms, &client_scopes, None);
    assert!(result.contains(&Permission::Admin));
    assert!(result.contains(&Permission::User));
}

#[test]
fn scenario_token_resource_expands_client_scopes() {
    let requested = vec!["user".to_string(), "admin".to_string()];
    let user_perms = vec![Permission::Admin, Permission::User];
    let client_scopes = vec!["user".to_string()];
    let resource_scopes = vec!["user".to_string(), "admin".to_string()];

    let result = simulate_token_permission_resolution(
        &requested,
        &user_perms,
        &client_scopes,
        Some(&resource_scopes),
    );
    assert!(result.contains(&Permission::Admin));
    assert!(result.contains(&Permission::User));
}

#[test]
fn scenario_token_user_with_resource_still_limited() {
    let requested = vec!["user".to_string(), "admin".to_string()];
    let user_perms = vec![Permission::User];
    let client_scopes = vec!["user".to_string()];
    let resource_scopes = vec!["user".to_string(), "admin".to_string()];

    let result = simulate_token_permission_resolution(
        &requested,
        &user_perms,
        &client_scopes,
        Some(&resource_scopes),
    );
    assert_eq!(result, vec![Permission::User]);
    assert!(!result.contains(&Permission::Admin));
}

#[test]
fn scenario_full_claude_code_flow() {
    let reg_request: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Claude Code",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let client_scopes = simulate_determine_scopes(&reg_request);
    assert!(client_scopes.contains(&"user".to_string()));

    let mcp_server_scopes = "user admin";
    let requested = OAuthRepository::parse_scopes(mcp_server_scopes);

    let auth_result =
        simulate_authorize_scope_check(&client_scopes, &requested, Some(mcp_server_scopes));
    assert!(auth_result.is_ok(), "Authorization passes — only validates scopes are legitimate");

    let admin_user_perms = vec![Permission::Admin, Permission::User];
    let token_perms = simulate_token_permission_resolution(
        &requested,
        &admin_user_perms,
        &client_scopes,
        Some(&requested),
    );
    assert!(token_perms.contains(&Permission::Admin));
    assert!(token_perms.contains(&Permission::User));

    let regular_user_perms = vec![Permission::User];
    let limited_perms = simulate_token_permission_resolution(
        &requested,
        &regular_user_perms,
        &client_scopes,
        Some(&requested),
    );
    assert!(limited_perms.contains(&Permission::User));
    assert!(!limited_perms.contains(&Permission::Admin), "Regular user cannot get admin");
}

#[test]
fn scenario_full_mcp_inspector_flow() {
    let reg_request: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "MCP Inspector",
        "redirect_uris": ["http://localhost:5173/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "scope": "user admin",
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let client_scopes = simulate_determine_scopes(&reg_request);
    assert_eq!(client_scopes, vec!["user", "admin"]);

    let requested = vec!["user".to_string(), "admin".to_string()];
    let auth_result = simulate_authorize_scope_check(&client_scopes, &requested, None);
    auth_result.expect("expected success");
}

#[test]
fn scenario_full_generic_client_no_resource_no_scope() {
    let reg_request: DynamicRegistrationRequest =
        serde_json::from_str(r#"{
        "client_name": "Generic MCP Client",
        "redirect_uris": ["https://example.com/oauth/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "client_secret_post"
    }"#).unwrap();

    let client_scopes = simulate_determine_scopes(&reg_request);
    assert!(client_scopes.contains(&"user".to_string()));

    let requested = vec!["user".to_string(), "admin".to_string()];
    let auth_result = simulate_authorize_scope_check(&client_scopes, &requested, None);
    assert!(auth_result.is_ok(), "Authorization passes — scopes are valid system scopes");
}

#[test]
fn scenario_client_requests_scope_from_protected_resource_metadata() {
    let reg_request: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Smart Client",
        "redirect_uris": ["http://127.0.0.1:8080/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let _client_scopes = simulate_determine_scopes(&reg_request);

    let scopes_from_protected_resource = "user admin";
    let requested = OAuthRepository::parse_scopes(scopes_from_protected_resource);
    let auth_result = simulate_authorize_scope_check(&[], &requested, None);
    assert!(
        auth_result.is_ok(),
        "Any client can request scopes — authorization only validates they're legitimate"
    );
}

#[test]
fn scenario_partial_resource_coverage_still_passes() {
    let client_scopes = vec!["user".to_string()];
    let requested_scopes = vec!["user".to_string(), "admin".to_string()];
    let resource_scopes = Some("user");

    let result =
        simulate_authorize_scope_check(&client_scopes, &requested_scopes, resource_scopes);
    assert!(
        result.is_ok(),
        "Authorization only validates scopes are legitimate, not client ownership"
    );
}

#[test]
fn scenario_registration_with_invalid_scopes_rejected() {
    let request: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Bad Client",
        "redirect_uris": ["https://example.com/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "scope": "nonexistent_scope",
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let scopes = simulate_determine_scopes(&request);
    assert!(
        scopes.contains(&"user".to_string()),
        "Invalid scopes fall back to defaults"
    );
}

// ============================================================================
// End-to-End Flow Tests
//
// The OAuth flow has two security gates:
//   Gate 1: Authorization — validates requested scopes are legitimate system scopes
//   Gate 2: Token — intersects requested scopes with user's database permissions
//
// Client registration scopes are informational only (used as fallback).
// User database permissions are the sole security boundary.
// ============================================================================

#[test]
fn flow_new_client_no_scope_no_resource() {
    let reg: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "New MCP Client",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let client_scopes = simulate_determine_scopes(&reg);
    assert!(client_scopes.contains(&"user".to_string()), "Registration defaults to user");

    let requested = vec!["user".to_string(), "admin".to_string()];
    let auth = simulate_authorize_scope_check(&client_scopes, &requested, None);
    assert!(auth.is_ok(), "Authorization validates scopes are legitimate");

    let admin_user = vec![Permission::Admin, Permission::User];
    let token = simulate_token_permission_resolution(
        &requested, &admin_user, &client_scopes, None,
    );
    assert!(token.contains(&Permission::User), "Admin user gets user");
    assert!(token.contains(&Permission::Admin), "Admin user gets admin");
}

#[test]
fn flow_new_client_no_scope_with_resource() {
    let reg: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Claude Code",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let client_scopes = simulate_determine_scopes(&reg);

    let requested = vec!["user".to_string(), "admin".to_string()];
    let resource = "user admin";
    let auth = simulate_authorize_scope_check(&client_scopes, &requested, Some(resource));
    assert!(auth.is_ok(), "Authorization passes");

    let admin_user = vec![Permission::Admin, Permission::User];
    let token = simulate_token_permission_resolution(
        &requested, &admin_user, &client_scopes, Some(&requested),
    );
    assert!(token.contains(&Permission::Admin), "Admin user gets admin");
}

#[test]
fn flow_legacy_client_user_only_with_resource() {
    let client_scopes = vec!["user".to_string()];

    let requested = vec!["user".to_string(), "admin".to_string()];
    let resource = "user admin";
    let auth = simulate_authorize_scope_check(&client_scopes, &requested, Some(resource));
    assert!(auth.is_ok(), "Authorization passes");

    let admin_user = vec![Permission::Admin, Permission::User];
    let resource_scopes = vec!["user".to_string(), "admin".to_string()];
    let token = simulate_token_permission_resolution(
        &requested, &admin_user, &client_scopes, Some(&resource_scopes),
    );
    assert!(token.contains(&Permission::Admin), "Admin user gets admin");
    assert!(token.contains(&Permission::User), "Admin user gets user");
}

#[test]
fn scenario_legacy_client_user_only_no_resource_passes_authorize() {
    let client_scopes = vec!["user".to_string()];

    let requested = vec!["user".to_string(), "admin".to_string()];
    let result = simulate_authorize_scope_check(&client_scopes, &requested, None);
    assert!(
        result.is_ok(),
        "Authorization passes — user permissions at token time are the security boundary"
    );
}

#[test]
fn flow_regular_user_cannot_get_admin_scope() {
    let reg: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Client",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#).unwrap();

    let client_scopes = simulate_determine_scopes(&reg);
    let requested = vec!["user".to_string(), "admin".to_string()];

    let auth = simulate_authorize_scope_check(&client_scopes, &requested, None);
    assert!(auth.is_ok(), "Authorization passes — scopes are valid");

    let regular_user = vec![Permission::User];
    let token = simulate_token_permission_resolution(
        &requested, &regular_user, &client_scopes, None,
    );
    assert!(token.contains(&Permission::User), "Gets user permission");
    assert!(!token.contains(&Permission::Admin), "Does NOT get admin — user lacks admin role");
}

#[test]
fn flow_webauthn_scope_fallback_uses_defaults() {
    let default_roles = OAuthRepository::get_default_roles();
    assert!(
        default_roles.contains(&"user".to_string()),
        "WebAuthn fallback must include user"
    );
}

#[test]
fn flow_scope_from_protected_resource_then_authorize_no_resource() {
    let reg: DynamicRegistrationRequest = serde_json::from_str(r#"{
        "client_name": "Smart Client",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#).unwrap();
    let _client_scopes = simulate_determine_scopes(&reg);

    let scopes_from_metadata = OAuthRepository::parse_scopes("user admin");
    let auth = simulate_authorize_scope_check(&[], &scopes_from_metadata, None);
    assert!(
        auth.is_ok(),
        "Authorization passes regardless of client scopes — only validates scopes are legitimate"
    );
}

#[test]
fn flow_token_scope_is_intersection_not_union() {
    let client_scopes = vec!["user".to_string(), "admin".to_string()];
    let requested = vec!["admin".to_string()];
    let user_perms = vec![Permission::Admin, Permission::User];

    let result = simulate_token_permission_resolution(
        &requested, &user_perms, &client_scopes, None,
    );
    assert_eq!(result, vec![Permission::Admin]);
    assert!(!result.contains(&Permission::User), "Only requested scopes should be in token");
}
