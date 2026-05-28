use std::str::FromStr;

use systemprompt_identifiers::ClientId;
use systemprompt_models::auth::{
    AuthError, AuthenticatedUser, Permission, PkceMethod, ResponseType, UserType,
};
use uuid::Uuid;

fn user_with_perms(perms: Vec<Permission>) -> AuthenticatedUser {
    AuthenticatedUser::new(
        Uuid::new_v4(),
        "alice".to_owned(),
        "alice@example.com".to_owned(),
        perms,
    )
}

#[test]
fn authenticated_user_new_defaults_roles_and_attributes() {
    let u = user_with_perms(vec![Permission::User]);
    assert!(u.roles.is_empty());
    assert!(u.attributes.is_empty());
    assert_eq!(u.permissions(), &[Permission::User]);
}

#[test]
fn authenticated_user_new_with_roles_carries_roles() {
    let u = AuthenticatedUser::new_with_roles(
        Uuid::new_v4(),
        "bob".to_owned(),
        "bob@x".to_owned(),
        vec![],
        vec!["editor".to_owned()],
    );
    assert_eq!(u.roles(), &["editor".to_owned()]);
}

#[test]
fn authenticated_user_with_attributes_round_trip() {
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("acme.desk".to_owned(), serde_json::json!("fixed-income"));
    let u = user_with_perms(vec![]).with_attributes(attrs.clone());
    assert_eq!(u.attributes(), &attrs);
}

#[test]
fn authenticated_user_has_permission_direct_and_implied() {
    let admin = user_with_perms(vec![Permission::Admin]);
    assert!(admin.has_permission(Permission::Admin));
    assert!(admin.has_permission(Permission::User));
    assert!(admin.is_admin());

    let plain = user_with_perms(vec![Permission::User]);
    assert!(!plain.has_permission(Permission::Admin));
    assert!(plain.has_permission(Permission::User));
    assert!(!plain.is_admin());
}

#[test]
fn authenticated_user_has_role_matches_exact() {
    let u = AuthenticatedUser::new_with_roles(
        Uuid::new_v4(),
        "u".to_owned(),
        "u@x".to_owned(),
        vec![],
        vec!["A".to_owned(), "B".to_owned()],
    );
    assert!(u.has_role("A"));
    assert!(u.has_role("B"));
    assert!(!u.has_role("a"));
    assert!(!u.has_role("C"));
}

#[test]
fn authenticated_user_user_type_derives_from_permissions() {
    let u = user_with_perms(vec![Permission::A2a]);
    assert_eq!(u.user_type(), UserType::A2a);
    let u = user_with_perms(vec![]);
    assert_eq!(u.user_type(), UserType::Anon);
}

#[test]
fn auth_error_displays_have_useful_text() {
    assert!(
        AuthError::InvalidTokenFormat
            .to_string()
            .contains("Invalid")
    );
    assert!(AuthError::TokenExpired.to_string().contains("expired"));
    assert!(
        AuthError::InvalidSignature
            .to_string()
            .contains("signature")
    );
    assert!(AuthError::UserNotFound.to_string().contains("User"));
    assert!(
        AuthError::InsufficientPermissions
            .to_string()
            .contains("permission")
    );
    assert!(
        AuthError::AuthenticationFailed {
            message: "bad pwd".to_owned()
        }
        .to_string()
        .contains("bad pwd")
    );
    assert!(
        AuthError::InvalidRequest {
            reason: "missing".to_owned()
        }
        .to_string()
        .contains("missing")
    );
    assert!(AuthError::MissingState.to_string().contains("CSRF"));
    assert!(
        AuthError::InvalidRedirectUri
            .to_string()
            .contains("Redirect")
    );
    assert!(
        AuthError::MissingCodeChallenge
            .to_string()
            .contains("code_challenge")
    );
    assert!(
        AuthError::WeakPkceMethod {
            method: "plain".to_owned()
        }
        .to_string()
        .contains("plain")
    );
    assert!(
        AuthError::ClientNotFound {
            client_id: ClientId::new("c1")
        }
        .to_string()
        .contains("c1")
    );
    assert!(
        AuthError::InvalidScope {
            scope: "x".to_owned()
        }
        .to_string()
        .contains("x")
    );
    assert!(
        AuthError::UnauthenticatedRevocation
            .to_string()
            .contains("revocation")
    );
    assert!(AuthError::InvalidRpId.to_string().contains("RP"));
    assert!(
        AuthError::RegistrationFailed {
            reason: "bad".to_owned()
        }
        .to_string()
        .contains("bad")
    );
    assert!(
        AuthError::Internal("x".to_owned())
            .to_string()
            .contains("x")
    );
}

#[test]
fn pkce_method_round_trips_s256() {
    assert_eq!(PkceMethod::from_str("S256").unwrap(), PkceMethod::S256);
    assert_eq!(PkceMethod::S256.to_string(), "S256");
}

#[test]
fn pkce_method_rejects_plain_as_weak() {
    let err = PkceMethod::from_str("plain").unwrap_err();
    assert!(matches!(err, AuthError::WeakPkceMethod { .. }));
}

#[test]
fn pkce_method_rejects_unknown_as_invalid_request() {
    let err = PkceMethod::from_str("other").unwrap_err();
    assert!(matches!(err, AuthError::InvalidRequest { .. }));
}

#[test]
fn response_type_round_trips_code_and_token() {
    assert_eq!(ResponseType::from_str("code").unwrap(), ResponseType::Code);
    assert_eq!(
        ResponseType::from_str("token").unwrap(),
        ResponseType::Token
    );
    assert_eq!(ResponseType::Code.to_string(), "code");
    assert_eq!(ResponseType::Token.to_string(), "token");
}

#[test]
fn response_type_rejects_unknown() {
    let err = ResponseType::from_str("garbage").unwrap_err();
    assert!(matches!(err, AuthError::InvalidRequest { .. }));
}
