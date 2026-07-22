//! Tests for `admin session show` session-info projection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{Duration, Utc};
use systemprompt_cli::admin::session::show::{missing_active_session, session_info};
use systemprompt_cloud::{CliSession, SessionIdentity};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, UserId};
use systemprompt_models::auth::UserType;

fn session(profile: &str) -> CliSession {
    CliSession::builder(
        ProfileName::new(profile),
        SessionToken::new("tok"),
        SessionId::generate(),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new("user-session-show"),
            Email::new("a@b.test"),
            UserType::Admin,
        ),
    )
    .build()
}

#[test]
fn local_key_is_displayed_as_local_and_tenant_prefix_is_stripped() {
    let s = session("alpha");
    assert_eq!(session_info("local", &s, true).key, "local");
    assert_eq!(session_info("tenant_t1", &s, false).key, "t1");
    assert_eq!(session_info("weird-key", &s, false).key, "weird-key");
}

#[test]
fn fresh_session_reports_expiry_countdown_and_no_stale_warning() {
    let s = session("alpha");
    let info = session_info("local", &s, true);

    assert!(info.is_active);
    assert!(!info.is_expired);
    let expires = info.expires_in.unwrap();
    assert!(expires.ends_with('m'));
    assert!(info.stale_warning.is_none());
    assert_eq!(info.profile_name, "alpha");
    assert_eq!(info.user_email, "a@b.test");
    assert!(info.session_id.is_some());
    assert!(info.context_id.is_some());
}

#[test]
fn expired_session_has_no_countdown() {
    let mut s = session("alpha");
    s.expires_at = Utc::now() - Duration::hours(1);

    let info = session_info("local", &s, false);
    assert!(info.is_expired);
    assert!(info.expires_in.is_none());
}

#[test]
fn stale_context_warns_after_a_day() {
    let mut s = session("alpha");
    s.last_used = Utc::now() - Duration::hours(30);

    let info = session_info("local", &s, false);
    let warning = info.stale_warning.unwrap();
    assert!(warning.contains("30h ago"));
}

#[test]
fn missing_active_session_prefers_profile_name() {
    let info = missing_active_session(Some("tenant_t1"), Some("prod"));
    assert_eq!(info.key, "prod");
    assert!(info.is_active);
    assert!(info.stale_warning.unwrap().contains("No session"));
}

#[test]
fn missing_active_session_falls_back_to_key_forms() {
    assert_eq!(missing_active_session(Some("local"), None).key, "local");
    assert_eq!(missing_active_session(Some("tenant_t9"), None).key, "t9");
    assert_eq!(missing_active_session(None, None).key, "unknown");
}
