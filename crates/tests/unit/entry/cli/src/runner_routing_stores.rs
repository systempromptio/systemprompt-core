//! Remote routing reads its tenant and session from the stores under the
//! profile's own system root. These drive that root into a temporary directory
//! so the successful reads are exercised without touching the developer's
//! `.systemprompt` directory.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use chrono::Duration;
use systemprompt_cli::runner::routing::{load_session_for_key, resolve_tenant};
use systemprompt_cloud::{
    CliSession, SessionBinding, SessionIdentity, SessionKey, SessionStore, StoredTenant,
    TenantStore,
};
use systemprompt_identifiers::{
    ContextId, Email, ProfileName, SessionId, SessionToken, TenantId, UserId,
};
use systemprompt_models::Profile;
use systemprompt_models::auth::UserType;
use tempfile::TempDir;

const ISSUER: &str = "http://localhost:8080";

fn profile_rooted_at(root: &Path) -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = fs::read_to_string(&boot.profile_path).expect("read the fixture profile");
    let mut profile: Profile = serde_yaml::from_str(&yaml).expect("parse the fixture profile");
    profile.paths.system = root.display().to_string();
    profile.security.issuer = ISSUER.to_owned();
    profile
}

fn project_root() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join(".systemprompt")).unwrap();
    fs::create_dir_all(dir.path().join("services")).unwrap();
    dir
}

fn session_for(key: &SessionKey, ttl: Duration) -> CliSession {
    CliSession::builder(
        SessionBinding::new(ProfileName::new("routed"), ISSUER.to_owned()),
        SessionToken::new("token-for-routing"),
        SessionId::new("session-for-routing"),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new(format!("user_{}", uuid::Uuid::new_v4().simple())),
            Email::new("router@routing.invalid"),
            UserType::User,
        ),
    )
    .with_session_key(key)
    .with_ttl(ttl)
    .build()
}

#[test]
fn a_synced_tenant_is_read_back_from_the_profiles_own_system_root() {
    let root = project_root();
    let profile = profile_rooted_at(root.path());
    let id = TenantId::new("tenant_routed");

    let mut tenant = StoredTenant::new(id.clone(), "Routed".to_owned());
    tenant.hostname = Some("routed.example.invalid".to_owned());
    TenantStore::new(vec![tenant])
        .save_to_path(&root.path().join(".systemprompt").join("tenants.json"))
        .expect("the tenant store is written under the profile root");

    let found = resolve_tenant(&profile, &id).expect("a synced tenant resolves");

    assert_eq!(found.id.as_str(), id.as_str());
    assert_eq!(
        found.hostname.as_deref(),
        Some("routed.example.invalid"),
        "the hostname routing dials must come back from the store"
    );
}

#[test]
fn a_store_holding_other_tenants_does_not_resolve_the_requested_one() {
    let root = project_root();
    let profile = profile_rooted_at(root.path());

    TenantStore::new(vec![StoredTenant::new(
        TenantId::new("tenant_other"),
        "Other".to_owned(),
    )])
    .save_to_path(&root.path().join(".systemprompt").join("tenants.json"))
    .unwrap();

    let err = resolve_tenant(&profile, &TenantId::new("tenant_routed"))
        .expect_err("a tenant absent from a loadable store is not resolvable");

    let message = format!("{err:#}");
    assert!(
        message.contains("not found in local tenant store"),
        "a loadable store missing the tenant must not be reported as a sync failure, got: \
         {message}"
    );
}

#[test]
fn a_valid_stored_session_supplies_the_token_and_context_for_the_remote_call() {
    let root = project_root();
    let profile = profile_rooted_at(root.path());
    let key = SessionKey::Tenant(TenantId::new("tenant_routed"));
    let sessions_dir = root.path().join(".systemprompt").join("sessions");

    let session = session_for(&key, Duration::hours(1));
    let mut store = SessionStore::new();
    store.upsert_session(&key, session.clone());
    store.save(&sessions_dir).unwrap();

    let loaded = load_session_for_key(&profile, &key, ISSUER).expect("a valid session resolves");

    assert_eq!(
        loaded.session_token.as_str(),
        session.session_token.as_str()
    );
    assert_eq!(loaded.context_id.as_str(), session.context_id.as_str());
}

#[test]
fn a_session_stored_under_a_different_issuer_is_not_reused() {
    let root = project_root();
    let profile = profile_rooted_at(root.path());
    let key = SessionKey::Tenant(TenantId::new("tenant_routed"));
    let sessions_dir = root.path().join(".systemprompt").join("sessions");

    let mut store = SessionStore::new();
    store.upsert_session(&key, session_for(&key, Duration::hours(1)));
    store.save(&sessions_dir).unwrap();

    let err = load_session_for_key(&profile, &key, "https://other-issuer.invalid")
        .expect_err("a session minted by another issuer must not authenticate a remote call");

    assert!(
        format!("{err:#}").contains("admin session login"),
        "the failure must name the login command"
    );
}

#[test]
fn an_expired_session_is_not_reused() {
    let root = project_root();
    let profile = profile_rooted_at(root.path());
    let key = SessionKey::Tenant(TenantId::new("tenant_routed"));
    let sessions_dir = root.path().join(".systemprompt").join("sessions");

    let mut store = SessionStore::new();
    store.upsert_session(&key, session_for(&key, Duration::hours(-1)));
    store.save(&sessions_dir).unwrap();

    load_session_for_key(&profile, &key, ISSUER)
        .expect_err("an expired session must not be handed to the remote executor");
}
