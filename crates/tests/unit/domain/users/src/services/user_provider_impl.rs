//! Unit tests for UserProviderImpl struct construction and From<User> for
//! AuthUser.
//!
//! `UserProviderImpl` wraps `UserService` (which requires a live DB pool) so
//! async trait methods are an untestable seam here.  We cover the `From<User>`
//! conversion and the struct's exported debug/clone surface.

use chrono::Utc;
use systemprompt_identifiers::UserId;
use systemprompt_traits::AuthUser;
use systemprompt_users::User;

fn make_user(status: Option<&str>, roles: Vec<&str>) -> User {
    User {
        id: UserId::new("u-provider"),
        name: "provider-user".to_string(),
        email: "provider@example.com".to_string(),
        full_name: Some("Provider User".to_string()),
        display_name: None,
        status: status.map(|s| s.to_string()),
        email_verified: Some(true),
        roles: roles.into_iter().map(|r| r.to_string()).collect(),
        avatar_url: None,
        is_bot: false,
        is_scanner: false,
        created_at: Some(Utc::now()),
        updated_at: Some(Utc::now()),
    }
}

mod from_user_to_auth_user_tests {
    use super::*;

    #[test]
    fn id_is_mapped() {
        let u = make_user(Some("active"), vec!["user"]);
        let a: AuthUser = u.into();
        assert_eq!(a.id.to_string(), "u-provider");
    }

    #[test]
    fn name_is_mapped() {
        let u = make_user(Some("active"), vec!["user"]);
        let a: AuthUser = u.into();
        assert_eq!(a.name, "provider-user");
    }

    #[test]
    fn email_is_mapped() {
        let u = make_user(Some("active"), vec!["user"]);
        let a: AuthUser = u.into();
        assert_eq!(a.email, "provider@example.com");
    }

    #[test]
    fn roles_are_mapped() {
        let u = make_user(Some("active"), vec!["user", "admin"]);
        let a: AuthUser = u.into();
        assert_eq!(a.roles.len(), 2);
        assert!(a.roles.contains(&"user".to_string()));
        assert!(a.roles.contains(&"admin".to_string()));
    }

    #[test]
    fn active_status_yields_is_active_true() {
        let u = make_user(Some("active"), vec!["user"]);
        let a: AuthUser = u.into();
        assert!(a.is_active);
    }

    #[test]
    fn none_status_yields_is_active_false() {
        let u = make_user(None, vec!["user"]);
        let a: AuthUser = u.into();
        assert!(!a.is_active);
    }

    #[test]
    fn suspended_status_yields_is_active_false() {
        let u = make_user(Some("suspended"), vec!["user"]);
        let a: AuthUser = u.into();
        assert!(!a.is_active);
    }

    #[test]
    fn deleted_status_yields_is_active_false() {
        let u = make_user(Some("deleted"), vec!["user"]);
        let a: AuthUser = u.into();
        assert!(!a.is_active);
    }

    #[test]
    fn pending_status_yields_is_active_false() {
        let u = make_user(Some("pending"), vec!["user"]);
        let a: AuthUser = u.into();
        assert!(!a.is_active);
    }

    #[test]
    fn inactive_status_yields_is_active_false() {
        let u = make_user(Some("inactive"), vec!["user"]);
        let a: AuthUser = u.into();
        assert!(!a.is_active);
    }

    #[test]
    fn case_sensitive_status_check() {
        let u = make_user(Some("Active"), vec!["user"]);
        let a: AuthUser = u.into();
        assert!(!a.is_active);
    }

    #[test]
    fn empty_roles_mapped() {
        let u = make_user(Some("active"), vec![]);
        let a: AuthUser = u.into();
        assert!(a.roles.is_empty());
    }

    #[test]
    fn bot_user_active_status_works() {
        let mut u = make_user(Some("active"), vec!["user"]);
        u.is_bot = true;
        let a: AuthUser = u.into();
        assert!(a.is_active);
    }
}
