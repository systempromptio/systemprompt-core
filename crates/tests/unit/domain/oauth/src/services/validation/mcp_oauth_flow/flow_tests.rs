use std::str::FromStr;
use systemprompt_models::auth::Permission;
use systemprompt_models::oauth::OAuthServerConfig;
use systemprompt_oauth::DynamicRegistrationRequest;
use systemprompt_oauth::repository::OAuthRepository;

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
                    user_permissions
                        .iter()
                        .filter(|p| p.is_user_role())
                        .copied(),
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
fn flow_new_client_no_scope_no_resource() {
    let reg: DynamicRegistrationRequest = serde_json::from_str(
        r#"{
        "client_name": "New MCP Client",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#,
    )
    .unwrap();

    let client_scopes = simulate_determine_scopes(&reg);
    assert!(
        client_scopes.contains(&"user".to_string()),
        "Registration defaults to user"
    );

    let requested = vec!["user".to_string(), "admin".to_string()];
    let auth = simulate_authorize_scope_check(&client_scopes, &requested, None);
    assert!(
        auth.is_ok(),
        "Authorization validates scopes are legitimate"
    );

    let admin_user = vec![Permission::Admin, Permission::User];
    let token = simulate_token_permission_resolution(&requested, &admin_user, &client_scopes, None);
    assert!(token.contains(&Permission::User), "Admin user gets user");
    assert!(token.contains(&Permission::Admin), "Admin user gets admin");
}

#[test]
fn flow_new_client_no_scope_with_resource() {
    let reg: DynamicRegistrationRequest = serde_json::from_str(
        r#"{
        "client_name": "Claude Code",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#,
    )
    .unwrap();

    let client_scopes = simulate_determine_scopes(&reg);

    let requested = vec!["user".to_string(), "admin".to_string()];
    let resource = "user admin";
    let auth = simulate_authorize_scope_check(&client_scopes, &requested, Some(resource));
    assert!(auth.is_ok(), "Authorization passes");

    let admin_user = vec![Permission::Admin, Permission::User];
    let token = simulate_token_permission_resolution(
        &requested,
        &admin_user,
        &client_scopes,
        Some(&requested),
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
        &requested,
        &admin_user,
        &client_scopes,
        Some(&resource_scopes),
    );
    assert!(token.contains(&Permission::Admin), "Admin user gets admin");
    assert!(token.contains(&Permission::User), "Admin user gets user");
}

#[test]
fn flow_regular_user_cannot_get_admin_scope() {
    let reg: DynamicRegistrationRequest = serde_json::from_str(
        r#"{
        "client_name": "Client",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#,
    )
    .unwrap();

    let client_scopes = simulate_determine_scopes(&reg);
    let requested = vec!["user".to_string(), "admin".to_string()];

    let auth = simulate_authorize_scope_check(&client_scopes, &requested, None);
    assert!(auth.is_ok(), "Authorization passes — scopes are valid");

    let regular_user = vec![Permission::User];
    let token =
        simulate_token_permission_resolution(&requested, &regular_user, &client_scopes, None);
    assert!(token.contains(&Permission::User), "Gets user permission");
    assert!(
        !token.contains(&Permission::Admin),
        "Does NOT get admin — user lacks admin role"
    );
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
    let reg: DynamicRegistrationRequest = serde_json::from_str(
        r#"{
        "client_name": "Smart Client",
        "redirect_uris": ["http://127.0.0.1:3000/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "token_endpoint_auth_method": "none"
    }"#,
    )
    .unwrap();
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

    let result =
        simulate_token_permission_resolution(&requested, &user_perms, &client_scopes, None);
    assert_eq!(result, vec![Permission::Admin]);
    assert!(
        !result.contains(&Permission::User),
        "Only requested scopes should be in token"
    );
}

#[test]
fn scenario_partial_resource_coverage_still_passes() {
    let client_scopes = vec!["user".to_string()];
    let requested_scopes = vec!["user".to_string(), "admin".to_string()];
    let resource_scopes = Some("user");

    let result = simulate_authorize_scope_check(&client_scopes, &requested_scopes, resource_scopes);
    assert!(
        result.is_ok(),
        "Authorization only validates scopes are legitimate, not client ownership"
    );
}

#[test]
fn scenario_registration_with_invalid_scopes_rejected() {
    let request: DynamicRegistrationRequest = serde_json::from_str(
        r#"{
        "client_name": "Bad Client",
        "redirect_uris": ["https://example.com/callback"],
        "grant_types": ["authorization_code"],
        "response_types": ["code"],
        "scope": "nonexistent_scope",
        "token_endpoint_auth_method": "none"
    }"#,
    )
    .unwrap();

    let scopes = simulate_determine_scopes(&request);
    assert!(
        scopes.contains(&"user".to_string()),
        "Invalid scopes fall back to defaults"
    );
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
