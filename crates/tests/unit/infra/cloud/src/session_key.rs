//! Unit tests for SessionKey

use systemprompt_cloud::cli_session::{LOCAL_SESSION_KEY, SessionKey};
use systemprompt_identifiers::TenantId;

#[test]
fn test_local_session_key_constant() {
    assert_eq!(LOCAL_SESSION_KEY, "local");
}

#[test]
fn test_session_key_from_tenant_id_none() {
    let key = SessionKey::from_tenant_id(None);
    assert!(matches!(key, SessionKey::Local));
}

#[test]
fn test_session_key_from_tenant_id_some() {
    let key = SessionKey::from_tenant_id(Some("tenant-123"));
    assert!(matches!(key, SessionKey::Tenant(_)));
}

#[test]
fn test_session_key_as_storage_key_local() {
    let key = SessionKey::Local;
    assert_eq!(key.as_storage_key(), "local");
}

#[test]
fn test_session_key_as_storage_key_tenant() {
    let key = SessionKey::Tenant(TenantId::new("tenant-456"));
    let storage_key = key.as_storage_key();
    assert_eq!(storage_key, "tenant_tenant-456");
}

#[test]
fn test_session_key_tenant_id_local() {
    let key = SessionKey::Local;
    assert!(key.tenant_id().is_none());
}

#[test]
fn test_session_key_tenant_id_tenant() {
    let tenant_id = TenantId::new("tenant-789");
    let key = SessionKey::Tenant(tenant_id.clone());
    assert_eq!(key.tenant_id(), Some(&tenant_id));
}

#[test]
fn test_session_key_tenant_id_str_local() {
    let key = SessionKey::Local;
    assert!(key.tenant_id_str().is_none());
}

#[test]
fn test_session_key_tenant_id_str_tenant() {
    let key = SessionKey::Tenant(TenantId::new("my-tenant"));
    assert_eq!(key.tenant_id_str(), Some("my-tenant"));
}

#[test]
fn test_session_key_is_local_true() {
    let key = SessionKey::Local;
    assert!(key.is_local());
}

#[test]
fn test_session_key_is_local_false() {
    let key = SessionKey::Tenant(TenantId::new("tenant"));
    assert!(!key.is_local());
}

#[test]
fn test_session_key_display_local() {
    let key = SessionKey::Local;
    let display = format!("{}", key);
    assert_eq!(display, "local");
}

#[test]
fn test_session_key_display_tenant() {
    let key = SessionKey::Tenant(TenantId::new("prod-tenant"));
    let display = format!("{}", key);
    assert_eq!(display, "tenant:prod-tenant");
}

#[test]
fn test_session_key_debug_local() {
    let key = SessionKey::Local;
    let debug = format!("{:?}", key);
    assert!(debug.contains("Local"));
}

#[test]
fn test_session_key_debug_tenant() {
    let key = SessionKey::Tenant(TenantId::new("debug-tenant"));
    let debug = format!("{:?}", key);
    assert!(debug.contains("Tenant"));
}

#[test]
fn test_session_key_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(SessionKey::Local);
    set.insert(SessionKey::Tenant(TenantId::new("t1")));
    set.insert(SessionKey::Tenant(TenantId::new("t2")));

    assert_eq!(set.len(), 3);
    assert!(set.contains(&SessionKey::Local));
}

#[test]
fn test_session_key_serialization_local() {
    let key = SessionKey::Local;
    let json = serde_json::to_string(&key).unwrap();
    assert!(json.contains("Local"));
}

#[test]
fn test_session_key_serialization_tenant() {
    let key = SessionKey::Tenant(TenantId::new("ser-tenant"));
    let json = serde_json::to_string(&key).unwrap();
    assert!(json.contains("Tenant"));
    assert!(json.contains("ser-tenant"));
}
