use chrono::{Duration, Utc};
use std::path::PathBuf;
use systemprompt_cloud::cli_session::{CliSession, CliSessionBuilder, SessionKey, SessionStore, LOCAL_SESSION_KEY};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, TenantId, UserId};
use systemprompt_models::auth::UserType;
use tempfile::TempDir;

fn test_builder(profile: &str) -> CliSessionBuilder {
    CliSessionBuilder::new(
        ProfileName::new(profile),
        SessionToken::new("token-abc"),
        SessionId::new("sid-001"),
        ContextId::new("ctx-001"),
    )
}

fn build_session(profile: &str) -> CliSession {
    test_builder(profile).build()
}

fn build_tenant_session(profile: &str, tenant: &str) -> CliSession {
    test_builder(profile)
        .with_tenant_key(TenantId::new(tenant))
        .build()
}

fn build_expired_session(profile: &str) -> CliSession {
    let mut session = build_session(profile);
    session.expires_at = Utc::now() - Duration::hours(1);
    session
}

#[test]
fn new_store_is_empty() {
    let store = SessionStore::new();

    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
    assert!(store.active_key.is_none());
    assert!(store.active_profile_name.is_none());
}

#[test]
fn default_store_matches_new() {
    let default_store = SessionStore::default();
    let new_store = SessionStore::new();

    assert_eq!(default_store.version, new_store.version);
    assert_eq!(default_store.sessions.len(), new_store.sessions.len());
    assert_eq!(default_store.active_key, new_store.active_key);
}

#[test]
fn upsert_session_adds_entry() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    let session = build_session("local-profile");

    store.upsert_session(&key, session);

    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());
}

#[test]
fn upsert_session_replaces_existing() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;

    store.upsert_session(&key, build_session("first"));
    store.upsert_session(&key, build_session("second"));

    assert_eq!(store.len(), 1);
    let retrieved = store.get_session(&key).unwrap();
    assert_eq!(retrieved.profile_name.as_str(), "second");
}

#[test]
fn get_session_returns_none_for_missing_key() {
    let store = SessionStore::new();
    let key = SessionKey::Local;

    assert!(store.get_session(&key).is_none());
}

#[test]
fn get_session_returns_existing() {
    let mut store = SessionStore::new();
    let key = SessionKey::Tenant(TenantId::new("t1"));
    store.upsert_session(&key, build_tenant_session("prof", "t1"));

    let result = store.get_session(&key);
    assert!(result.is_some());
    assert_eq!(result.unwrap().profile_name.as_str(), "prof");
}

#[test]
fn get_valid_session_returns_fresh_session() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("active"));

    assert!(store.get_valid_session(&key).is_some());
}

#[test]
fn get_valid_session_returns_none_for_expired() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_expired_session("expired"));

    assert!(store.get_valid_session(&key).is_none());
}

#[test]
fn get_valid_session_returns_none_for_empty_token() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    let session = CliSessionBuilder::new(
        ProfileName::new("no-creds"),
        SessionToken::new(""),
        SessionId::new("sid"),
        ContextId::new("ctx"),
    )
    .build();
    store.upsert_session(&key, session);

    assert!(store.get_valid_session(&key).is_none());
}

#[test]
fn get_valid_session_mut_returns_mutable_ref() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("mutable"));

    let session = store.get_valid_session_mut(&key).unwrap();
    session.set_context_id(ContextId::new("updated-ctx"));

    let retrieved = store.get_session(&key).unwrap();
    assert_eq!(retrieved.context_id.as_str(), "updated-ctx");
}

#[test]
fn get_valid_session_mut_returns_none_for_expired() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_expired_session("old"));

    assert!(store.get_valid_session_mut(&key).is_none());
}

#[test]
fn remove_session_returns_removed() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("to-remove"));

    let removed = store.remove_session(&key);
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().profile_name.as_str(), "to-remove");
    assert!(store.is_empty());
}

#[test]
fn remove_session_returns_none_for_missing() {
    let mut store = SessionStore::new();
    let key = SessionKey::Tenant(TenantId::new("nonexistent"));

    let removed = store.remove_session(&key);
    assert!(removed.is_none());
}

#[test]
fn set_active_stores_key() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("active"));
    store.set_active(&key);

    assert_eq!(store.active_key, Some(LOCAL_SESSION_KEY.to_string()));
}

#[test]
fn set_active_with_profile_stores_both() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("prof-name"));
    store.set_active_with_profile(&key, "prof-name");

    assert_eq!(store.active_key, Some(LOCAL_SESSION_KEY.to_string()));
    assert_eq!(store.active_profile_name, Some("prof-name".to_string()));
}

#[test]
fn set_active_with_profile_path_updates_session() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("path-prof"));

    store.set_active_with_profile_path(&key, "path-prof", PathBuf::from("/my/profile.yaml"));

    let session = store.get_session(&key).unwrap();
    assert_eq!(session.profile_path, Some(PathBuf::from("/my/profile.yaml")));
}

#[test]
fn set_active_with_profile_path_nonexistent_session() {
    let mut store = SessionStore::new();
    let key = SessionKey::Tenant(TenantId::new("ghost"));

    store.set_active_with_profile_path(&key, "ghost-prof", PathBuf::from("/ghost/path"));

    assert_eq!(store.active_key, Some("tenant_ghost".to_string()));
    assert_eq!(store.active_profile_name, Some("ghost-prof".to_string()));
}

#[test]
fn active_session_key_returns_none_when_unset() {
    let store = SessionStore::new();
    assert!(store.active_session_key().is_none());
}

#[test]
fn active_session_key_returns_local_for_local_key() {
    let mut store = SessionStore::new();
    store.active_key = Some(LOCAL_SESSION_KEY.to_string());

    let key = store.active_session_key().unwrap();
    assert!(matches!(key, SessionKey::Local));
}

#[test]
fn active_session_key_returns_tenant_for_tenant_key() {
    let mut store = SessionStore::new();
    store.active_key = Some("tenant_my-org".to_string());

    let key = store.active_session_key().unwrap();
    match key {
        SessionKey::Tenant(id) => assert_eq!(id.as_str(), "my-org"),
        SessionKey::Local => panic!("Expected Tenant variant"),
    }
}

#[test]
fn active_session_key_falls_back_to_local_for_unknown_prefix() {
    let mut store = SessionStore::new();
    store.active_key = Some("unknown-format".to_string());

    let key = store.active_session_key().unwrap();
    assert!(matches!(key, SessionKey::Local));
}

#[test]
fn active_session_returns_valid_session() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("active"));
    store.set_active(&key);

    let session = store.active_session();
    assert!(session.is_some());
    assert_eq!(session.unwrap().profile_name.as_str(), "active");
}

#[test]
fn active_session_returns_none_when_expired() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_expired_session("stale"));
    store.set_active(&key);

    assert!(store.active_session().is_none());
}

#[test]
fn active_session_returns_none_when_no_active_key() {
    let mut store = SessionStore::new();
    let key = SessionKey::Local;
    store.upsert_session(&key, build_session("exists"));

    assert!(store.active_session().is_none());
}

#[test]
fn prune_expired_removes_expired_sessions() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_expired_session("old"));
    store.upsert_session(
        &SessionKey::Tenant(TenantId::new("alive")),
        build_session("fresh"),
    );

    let pruned = store.prune_expired();

    assert_eq!(pruned, 1);
    assert_eq!(store.len(), 1);
}

#[test]
fn prune_expired_returns_zero_when_none_expired() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_session("ok"));

    let pruned = store.prune_expired();
    assert_eq!(pruned, 0);
}

#[test]
fn prune_expired_on_empty_store() {
    let mut store = SessionStore::new();
    let pruned = store.prune_expired();
    assert_eq!(pruned, 0);
}

#[test]
fn prune_expired_removes_all_when_all_expired() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_expired_session("a"));
    store.upsert_session(
        &SessionKey::Tenant(TenantId::new("b")),
        build_expired_session("b"),
    );

    let pruned = store.prune_expired();
    assert_eq!(pruned, 2);
    assert!(store.is_empty());
}

#[test]
fn find_by_profile_name_finds_matching() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_session("target"));

    let found = store.find_by_profile_name("target");
    assert!(found.is_some());
    assert_eq!(found.unwrap().profile_name.as_str(), "target");
}

#[test]
fn find_by_profile_name_returns_none_for_missing() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_session("other"));

    assert!(store.find_by_profile_name("missing").is_none());
}

#[test]
fn find_by_profile_name_skips_expired() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_expired_session("stale-prof"));

    assert!(store.find_by_profile_name("stale-prof").is_none());
}

#[test]
fn all_sessions_returns_all_entries() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_session("a"));
    store.upsert_session(
        &SessionKey::Tenant(TenantId::new("t")),
        build_session("b"),
    );

    let all = store.all_sessions();
    assert_eq!(all.len(), 2);
}

#[test]
fn all_sessions_empty_store() {
    let store = SessionStore::new();
    assert!(store.all_sessions().is_empty());
}

#[test]
fn len_tracks_insertions_and_removals() {
    let mut store = SessionStore::new();
    assert_eq!(store.len(), 0);

    store.upsert_session(&SessionKey::Local, build_session("x"));
    assert_eq!(store.len(), 1);

    store.upsert_session(
        &SessionKey::Tenant(TenantId::new("t1")),
        build_session("y"),
    );
    assert_eq!(store.len(), 2);

    store.remove_session(&SessionKey::Local);
    assert_eq!(store.len(), 1);
}

#[test]
fn multiple_tenant_sessions_coexist() {
    let mut store = SessionStore::new();
    let key_a = SessionKey::Tenant(TenantId::new("org-a"));
    let key_b = SessionKey::Tenant(TenantId::new("org-b"));

    store.upsert_session(&key_a, build_tenant_session("prof-a", "org-a"));
    store.upsert_session(&key_b, build_tenant_session("prof-b", "org-b"));

    assert_eq!(store.len(), 2);
    assert_eq!(
        store.get_session(&key_a).unwrap().profile_name.as_str(),
        "prof-a"
    );
    assert_eq!(
        store.get_session(&key_b).unwrap().profile_name.as_str(),
        "prof-b"
    );
}

#[test]
fn save_and_load_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().join("sessions");

    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_session("saved"));
    store.set_active(&SessionKey::Local);
    store.active_profile_name = Some("saved".to_string());
    store.save(&dir).unwrap();

    let loaded = SessionStore::load(&dir).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.active_key, Some(LOCAL_SESSION_KEY.to_string()));
    assert_eq!(loaded.active_profile_name, Some("saved".to_string()));
}

#[test]
fn load_returns_none_for_missing_dir() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().join("nonexistent");

    assert!(SessionStore::load(&dir).is_none());
}

#[test]
fn load_or_create_returns_new_when_missing() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().join("fresh");

    let store = SessionStore::load_or_create(&dir).unwrap();
    assert!(store.is_empty());
}

#[test]
fn load_or_create_returns_existing() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().join("existing");

    let mut original = SessionStore::new();
    original.upsert_session(&SessionKey::Local, build_session("existing"));
    original.save(&dir).unwrap();

    let loaded = SessionStore::load_or_create(&dir).unwrap();
    assert_eq!(loaded.len(), 1);
}

#[test]
fn save_creates_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().join("sessions");

    let store = SessionStore::new();
    store.save(&dir).unwrap();

    let gitignore = dir.join(".gitignore");
    assert!(gitignore.exists());
}

#[test]
fn save_creates_parent_directories() {
    let temp_dir = TempDir::new().unwrap();
    let dir = temp_dir.path().join("deep").join("nested").join("sessions");

    let store = SessionStore::new();
    store.save(&dir).unwrap();

    assert!(dir.join("index.json").exists());
}

#[test]
fn serde_roundtrip_preserves_all_fields() {
    let mut store = SessionStore::new();
    let key = SessionKey::Tenant(TenantId::new("serde-test"));
    let session = test_builder("serde-prof")
        .with_tenant_key(TenantId::new("serde-test"))
        .with_user(UserId::new("uid-99"), Email::new("serde@test.com"))
        .with_user_type(UserType::User)
        .with_profile_path("/serde/path.yaml")
        .build();
    store.upsert_session(&key, session);
    store.set_active_with_profile(&key, "serde-prof");

    let json = serde_json::to_string(&store).unwrap();
    let deserialized: SessionStore = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.len(), 1);
    assert_eq!(deserialized.active_key, store.active_key);
    assert_eq!(deserialized.active_profile_name, store.active_profile_name);

    let s = deserialized.get_session(&key).unwrap();
    assert_eq!(s.profile_name.as_str(), "serde-prof");
    assert_eq!(s.user_id, UserId::new("uid-99"));
    assert_eq!(s.user_email.as_str(), "serde@test.com");
    assert_eq!(s.user_type, UserType::User);
    assert_eq!(s.profile_path, Some(PathBuf::from("/serde/path.yaml")));
}

#[test]
fn serde_roundtrip_empty_store() {
    let store = SessionStore::new();
    let json = serde_json::to_string(&store).unwrap();
    let deserialized: SessionStore = serde_json::from_str(&json).unwrap();

    assert!(deserialized.is_empty());
    assert!(deserialized.active_key.is_none());
}

#[test]
fn updated_at_changes_on_upsert() {
    let mut store = SessionStore::new();
    let initial = store.updated_at;

    std::thread::sleep(std::time::Duration::from_millis(10));
    store.upsert_session(&SessionKey::Local, build_session("x"));

    assert!(store.updated_at > initial);
}

#[test]
fn updated_at_changes_on_remove() {
    let mut store = SessionStore::new();
    store.upsert_session(&SessionKey::Local, build_session("x"));
    let after_insert = store.updated_at;

    std::thread::sleep(std::time::Duration::from_millis(10));
    store.remove_session(&SessionKey::Local);

    assert!(store.updated_at > after_insert);
}

#[test]
fn updated_at_unchanged_on_failed_remove() {
    let mut store = SessionStore::new();
    let initial = store.updated_at;

    store.remove_session(&SessionKey::Local);

    assert_eq!(store.updated_at, initial);
}

#[test]
fn active_session_with_tenant_key() {
    let mut store = SessionStore::new();
    let key = SessionKey::Tenant(TenantId::new("active-tenant"));
    store.upsert_session(&key, build_tenant_session("t-prof", "active-tenant"));
    store.set_active(&key);

    let session = store.active_session().unwrap();
    assert_eq!(session.profile_name.as_str(), "t-prof");
}

#[test]
fn switching_active_session() {
    let mut store = SessionStore::new();
    let local_key = SessionKey::Local;
    let tenant_key = SessionKey::Tenant(TenantId::new("switched"));

    store.upsert_session(&local_key, build_session("local"));
    store.upsert_session(&tenant_key, build_tenant_session("tenant", "switched"));

    store.set_active(&local_key);
    assert_eq!(
        store.active_session().unwrap().profile_name.as_str(),
        "local"
    );

    store.set_active(&tenant_key);
    assert_eq!(
        store.active_session().unwrap().profile_name.as_str(),
        "tenant"
    );
}
