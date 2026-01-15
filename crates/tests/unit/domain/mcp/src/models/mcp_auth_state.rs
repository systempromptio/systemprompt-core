//! Unit tests for McpAuthState

use systemprompt_core_mcp::McpAuthState;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use uuid::Uuid;

fn create_test_user() -> AuthenticatedUser {
    AuthenticatedUser {
        id: Uuid::new_v4(),
        username: "test_user".to_string(),
        email: "test@example.com".to_string(),
        permissions: vec![Permission::Admin],
        roles: vec![],
    }
}

// ============================================================================
// McpAuthState is_authenticated Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_is_authenticated_true() {
    let user = create_test_user();
    let state = McpAuthState::Authenticated(user);
    assert!(state.is_authenticated());
}

#[test]
fn test_mcp_auth_state_is_authenticated_false() {
    let state = McpAuthState::Anonymous;
    assert!(!state.is_authenticated());
}

// ============================================================================
// McpAuthState is_anonymous Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_is_anonymous_true() {
    let state = McpAuthState::Anonymous;
    assert!(state.is_anonymous());
}

#[test]
fn test_mcp_auth_state_is_anonymous_false() {
    let user = create_test_user();
    let state = McpAuthState::Authenticated(user);
    assert!(!state.is_anonymous());
}

// ============================================================================
// McpAuthState user Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_user_some() {
    let user = create_test_user();
    let state = McpAuthState::Authenticated(user);

    let user_ref = state.user();
    assert!(user_ref.is_some());
    assert_eq!(user_ref.unwrap().username, "test_user");
}

#[test]
fn test_mcp_auth_state_user_none() {
    let state = McpAuthState::Anonymous;
    assert!(state.user().is_none());
}

// ============================================================================
// McpAuthState username Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_username_authenticated() {
    let user = create_test_user();
    let state = McpAuthState::Authenticated(user);
    assert_eq!(state.username(), "test_user");
}

#[test]
fn test_mcp_auth_state_username_anonymous() {
    let state = McpAuthState::Anonymous;
    assert_eq!(state.username(), "anonymous");
}

// ============================================================================
// McpAuthState has_permission Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_has_permission_anonymous_allowed() {
    let state = McpAuthState::Anonymous;
    assert!(state.has_permission(Permission::Anonymous));
}

#[test]
fn test_mcp_auth_state_has_permission_anonymous_denied() {
    let state = McpAuthState::Anonymous;
    assert!(!state.has_permission(Permission::Admin));
}

// ============================================================================
// McpAuthState Clone Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_clone_authenticated() {
    let user = create_test_user();
    let state = McpAuthState::Authenticated(user);
    let cloned = state.clone();

    assert!(cloned.is_authenticated());
    assert_eq!(state.username(), cloned.username());
}

#[test]
fn test_mcp_auth_state_clone_anonymous() {
    let state = McpAuthState::Anonymous;
    let cloned = state.clone();
    assert!(cloned.is_anonymous());
}

// ============================================================================
// McpAuthState Debug Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_debug_authenticated() {
    let user = create_test_user();
    let state = McpAuthState::Authenticated(user);
    let debug_str = format!("{:?}", state);
    assert!(debug_str.contains("Authenticated"));
}

#[test]
fn test_mcp_auth_state_debug_anonymous() {
    let state = McpAuthState::Anonymous;
    let debug_str = format!("{:?}", state);
    assert!(debug_str.contains("Anonymous"));
}

// ============================================================================
// McpAuthState Serialization Tests
// ============================================================================

#[test]
fn test_mcp_auth_state_serialize_authenticated() {
    let user = create_test_user();
    let state = McpAuthState::Authenticated(user);
    let json = serde_json::to_string(&state).unwrap();

    assert!(json.contains("Authenticated"));
    assert!(json.contains("test_user"));
}

#[test]
fn test_mcp_auth_state_serialize_anonymous() {
    let state = McpAuthState::Anonymous;
    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("Anonymous"));
}
