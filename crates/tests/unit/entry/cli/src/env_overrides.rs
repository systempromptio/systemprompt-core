//! Unit tests for the `env_overrides` module.
//!
//! All snapshots are built with `EnvOverrides::from_vars` so no test mutates
//! process-global environment variables.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::env_overrides::EnvOverrides;

#[test]
fn empty_iter_yields_all_unset() {
    let env = EnvOverrides::from_vars(std::iter::empty::<(String, String)>());
    assert!(env.output_format.is_none());
    assert!(env.log_level.is_none());
    assert!(!env.no_color);
    assert!(!env.non_interactive);
    assert!(env.profile.is_none());
    assert!(env.rust_log.is_none());
    assert!(!env.is_fly);
    assert!(!env.is_remote_cli);
    assert!(env.editor.is_none());
    assert!(env.database_url.is_none());
    assert!(env.services_path.is_none());
    assert!(env.session.user_id.is_none());
    assert!(env.session.session_id.is_none());
    assert!(env.session.context_id.is_none());
    assert!(env.session.auth_token.is_none());
}

#[test]
fn from_vars_maps_string_fields() {
    let env = EnvOverrides::from_vars([
        ("SYSTEMPROMPT_OUTPUT_FORMAT", "json"),
        ("SYSTEMPROMPT_LOG_LEVEL", "verbose"),
        ("SYSTEMPROMPT_PROFILE", "local"),
        ("RUST_LOG", "debug"),
        ("DATABASE_URL", "postgres://localhost/test"),
        ("SYSTEMPROMPT_SERVICES_PATH", "/srv/services"),
    ]);
    assert_eq!(env.output_format.as_deref(), Some("json"));
    assert_eq!(env.log_level.as_deref(), Some("verbose"));
    assert_eq!(env.profile.as_deref(), Some("local"));
    assert_eq!(env.rust_log.as_deref(), Some("debug"));
    assert_eq!(env.database_url.as_deref(), Some("postgres://localhost/test"));
    assert_eq!(env.services_path.as_deref(), Some("/srv/services"));
}

#[test]
fn from_vars_maps_presence_flags() {
    let env = EnvOverrides::from_vars([
        ("SYSTEMPROMPT_NON_INTERACTIVE", ""),
        ("FLY_APP_NAME", "my-app"),
        ("SYSTEMPROMPT_CLI_REMOTE", "1"),
    ]);
    assert!(env.non_interactive);
    assert!(env.is_fly);
    assert!(env.is_remote_cli);
    assert!(!env.no_color);
}

#[test]
fn no_color_set_by_either_variable() {
    let env = EnvOverrides::from_vars([("NO_COLOR", "1")]);
    assert!(env.no_color);

    let env = EnvOverrides::from_vars([("SYSTEMPROMPT_NO_COLOR", "1")]);
    assert!(env.no_color);
}

#[test]
fn editor_prefers_visual_over_editor() {
    let env = EnvOverrides::from_vars([("VISUAL", "nvim"), ("EDITOR", "vi")]);
    assert_eq!(env.editor.as_deref(), Some("nvim"));

    let env = EnvOverrides::from_vars([("EDITOR", "vi")]);
    assert_eq!(env.editor.as_deref(), Some("vi"));
}

#[test]
fn from_vars_maps_session_fields() {
    let env = EnvOverrides::from_vars([
        ("SYSTEMPROMPT_USER_ID", "user-1"),
        ("SYSTEMPROMPT_SESSION_ID", "session-1"),
        ("SYSTEMPROMPT_CONTEXT_ID", "context-1"),
        ("SYSTEMPROMPT_AUTH_TOKEN", "token-1"),
    ]);
    assert_eq!(env.session.user_id.as_deref(), Some("user-1"));
    assert_eq!(env.session.session_id.as_deref(), Some("session-1"));
    assert_eq!(env.session.context_id.as_deref(), Some("context-1"));
    assert_eq!(env.session.auth_token.as_deref(), Some("token-1"));
}

#[test]
fn unrelated_variables_are_ignored() {
    let env = EnvOverrides::from_vars([("PATH", "/usr/bin"), ("HOME", "/home/user")]);
    assert!(env.output_format.is_none());
    assert!(env.database_url.is_none());
    assert!(!env.is_fly);
}
