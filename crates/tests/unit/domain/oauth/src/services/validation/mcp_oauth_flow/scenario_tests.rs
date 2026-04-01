use std::str::FromStr;
use systemprompt_models::auth::Permission;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::DynamicRegistrationRequest;

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
    assert!(result.is_ok());
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
    assert!(result.is_ok());
}

#[test]
fn scenario_authorize_subset_of_client_scopes() {
    let client_scopes = vec!["user".to_string(), "admin".to_string()];
    let requested_scopes = vec!["user".to_string()];

    let result = simulate_authorize_scope_check(&client_scopes, &requested_scopes, None);
    assert!(result.is_ok());
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
    assert!(auth_result.is_ok());
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
