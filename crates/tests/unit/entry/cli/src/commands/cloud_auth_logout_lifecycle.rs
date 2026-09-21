//! Cloud logout cancellation and tenant-session cleanup lifecycle.

use std::path::PathBuf;

use chrono::Duration;
use systemprompt_cli::cloud::auth::{self, AuthCommands, LogoutArgs};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat, ScriptedPrompter};
use systemprompt_cloud::{
    CliSession, CloudPath, SessionBinding, SessionIdentity, SessionKey, SessionStore,
    get_cloud_paths,
};
use systemprompt_identifiers::{
    ContextId, Email, ProfileName, SessionId, SessionToken, TenantId, UserId,
};
use systemprompt_models::auth::UserType;

struct CwdGuard(PathBuf);
impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn session(key: &SessionKey, label: &str) -> CliSession {
    CliSession::builder(
        SessionBinding::new(
            ProfileName::try_new("coverage").expect("profile"),
            "https://issuer.example.invalid".to_owned(),
        ),
        SessionToken::new(format!("token-{label}")),
        SessionId::new(format!("session-{label}")),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new(format!("user-{label}")),
            Email::try_new(format!("{label}@example.invalid")).expect("email"),
            UserType::User,
        ),
    )
    .with_session_key(key)
    .with_ttl(Duration::hours(1))
    .build()
}

#[tokio::test]
async fn cancellation_preserves_cloud_state_and_confirmed_logout_removes_only_tenant_sessions() {
    let root = tempfile::tempdir().expect("owned cloud project");
    std::fs::create_dir_all(root.path().join(".systemprompt")).expect("cloud directory");
    let prior = std::env::current_dir().expect("current directory");
    std::env::set_current_dir(root.path()).expect("enter owned project");
    let _cwd = CwdGuard(prior);
    let paths = get_cloud_paths();
    let credentials = paths.resolve(CloudPath::Credentials);
    let tenants = paths.resolve(CloudPath::Tenants);
    let sessions_dir = paths.resolve(CloudPath::SessionsDir);
    for path in [&credentials, &tenants, &sessions_dir] {
        assert!(
            path.starts_with(root.path()),
            "resolved path escaped owned root: {}",
            path.display()
        );
    }
    std::fs::write(&credentials, b"owned credentials bytes").expect("credentials fixture");
    std::fs::write(&tenants, b"owned tenants bytes").expect("tenant fixture");
    let local = SessionKey::Local;
    let tenant_a = SessionKey::Tenant(TenantId::new("tenant-a"));
    let tenant_b = SessionKey::Tenant(TenantId::new("tenant-b"));
    let mut store = SessionStore::new();
    store.upsert_session(&local, session(&local, "local"));
    store.upsert_session(&tenant_a, session(&tenant_a, "tenant-a"));
    store.upsert_session(&tenant_b, session(&tenant_b, "tenant-b"));
    store.set_active_with_profile(&tenant_a, "coverage");
    let local_before =
        serde_json::to_value(store.get_session(&local).expect("local session before"))
            .expect("serialize local session");
    store.save(&sessions_dir).expect("persist mixed sessions");
    let credentials_before = std::fs::read(&credentials).expect("credentials before");
    let tenants_before = std::fs::read(&tenants).expect("tenants before");
    let sessions_before = std::fs::read(sessions_dir.join("index.json")).expect("sessions before");

    let cancelled = CommandContext::new(
        CliConfig::new()
            .with_interactive(true)
            .with_assume_terminal(true)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_prompter(Box::new(ScriptedPrompter::new(["no"])));
    auth::execute(AuthCommands::Logout(LogoutArgs { yes: false }), &cancelled)
        .await
        .expect("cancel logout");
    assert_eq!(
        std::fs::read(&credentials).expect("credentials remain"),
        credentials_before
    );
    assert_eq!(
        std::fs::read(&tenants).expect("tenants remain"),
        tenants_before
    );
    assert_eq!(
        std::fs::read(sessions_dir.join("index.json")).expect("sessions remain"),
        sessions_before
    );

    let confirmed = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    );
    auth::execute(AuthCommands::Logout(LogoutArgs { yes: true }), &confirmed)
        .await
        .expect("confirmed logout");
    assert!(!credentials.exists());
    assert!(!tenants.exists());
    let remaining = SessionStore::load(&sessions_dir)
        .expect("load remaining sessions")
        .expect("session store remains");
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        serde_json::to_value(
            remaining
                .get_session(&SessionKey::Local)
                .expect("local remains")
        )
        .expect("serialize remaining local"),
        local_before
    );
    assert!(remaining.active_key.is_none());
    assert!(remaining.active_profile_name.is_none());
    assert!(remaining.get_session(&tenant_a).is_none());
    assert!(remaining.get_session(&tenant_b).is_none());
}
