//! Unit tests for cloud context types
//!
//! Tests cover:
//! - ResolvedTenant creation from StoredTenant
//! - ResolvedTenant field accessors

use systemprompt_cloud::{ResolvedTenant, StoredTenant};
use systemprompt_cloud::tenants::NewCloudTenantParams;

// ============================================================================
// ResolvedTenant From<StoredTenant> Tests
// ============================================================================

#[test]
fn test_resolved_tenant_from_stored_tenant_minimal() {
    let stored = StoredTenant::new("t-123".to_string(), "Minimal Tenant".to_string());
    let resolved: ResolvedTenant = stored.into();

    assert_eq!(resolved.id, "t-123");
    assert_eq!(resolved.name, "Minimal Tenant");
    assert!(resolved.app_id.is_none());
    assert!(resolved.hostname.is_none());
    assert!(resolved.region.is_none());
}

#[test]
fn test_resolved_tenant_from_stored_tenant_full() {
    let params = NewCloudTenantParams {
        id: "cloud-123".to_string(),
        name: "Full Cloud Tenant".to_string(),
        app_id: Some("app-456".to_string()),
        hostname: Some("tenant.example.com".to_string()),
        region: Some("iad".to_string()),
        database_url: "postgres://db".to_string(),
    };
    let stored = StoredTenant::new_cloud(params);
    let resolved: ResolvedTenant = stored.into();

    assert_eq!(resolved.id, "cloud-123");
    assert_eq!(resolved.name, "Full Cloud Tenant");
    assert_eq!(resolved.app_id, Some("app-456".to_string()));
    assert_eq!(resolved.hostname, Some("tenant.example.com".to_string()));
    assert_eq!(resolved.region, Some("iad".to_string()));
}

#[test]
fn test_resolved_tenant_from_local_tenant() {
    let stored = StoredTenant::new_local(
        "local-1".to_string(),
        "Local Dev".to_string(),
        "postgres://localhost/dev".to_string(),
    );
    let resolved: ResolvedTenant = stored.into();

    assert_eq!(resolved.id, "local-1");
    assert_eq!(resolved.name, "Local Dev");
    // Local tenants typically don't have app_id, hostname, or region
    assert!(resolved.app_id.is_none());
    assert!(resolved.hostname.is_none());
    assert!(resolved.region.is_none());
}

// ============================================================================
// ResolvedTenant Clone Tests
// ============================================================================

#[test]
fn test_resolved_tenant_clone() {
    let stored = StoredTenant::new("t-clone".to_string(), "Clone Test".to_string());
    let original: ResolvedTenant = stored.into();
    let cloned = original.clone();

    assert_eq!(cloned.id, original.id);
    assert_eq!(cloned.name, original.name);
    assert_eq!(cloned.app_id, original.app_id);
    assert_eq!(cloned.hostname, original.hostname);
    assert_eq!(cloned.region, original.region);
}

// ============================================================================
// ResolvedTenant Debug Tests
// ============================================================================

#[test]
fn test_resolved_tenant_debug() {
    let stored = StoredTenant::new("t-debug".to_string(), "Debug Test".to_string());
    let resolved: ResolvedTenant = stored.into();
    let debug_str = format!("{:?}", resolved);

    assert!(debug_str.contains("ResolvedTenant"));
    assert!(debug_str.contains("t-debug"));
}

// ============================================================================
// ResolvedTenant Field Preservation Tests
// ============================================================================

#[test]
fn test_resolved_tenant_preserves_app_id() {
    let mut stored = StoredTenant::new("t-1".to_string(), "Test".to_string());
    stored.app_id = Some("preserved-app".to_string());

    let resolved: ResolvedTenant = stored.into();
    assert_eq!(resolved.app_id, Some("preserved-app".to_string()));
}

#[test]
fn test_resolved_tenant_preserves_hostname() {
    let mut stored = StoredTenant::new("t-1".to_string(), "Test".to_string());
    stored.hostname = Some("preserved.example.com".to_string());

    let resolved: ResolvedTenant = stored.into();
    assert_eq!(resolved.hostname, Some("preserved.example.com".to_string()));
}

#[test]
fn test_resolved_tenant_preserves_region() {
    let mut stored = StoredTenant::new("t-1".to_string(), "Test".to_string());
    stored.region = Some("lhr".to_string());

    let resolved: ResolvedTenant = stored.into();
    assert_eq!(resolved.region, Some("lhr".to_string()));
}

// ============================================================================
// ResolvedTenant with Various ID Formats Tests
// ============================================================================

#[test]
fn test_resolved_tenant_with_uuid_id() {
    let stored = StoredTenant::new(
        "550e8400-e29b-41d4-a716-446655440000".to_string(),
        "UUID Tenant".to_string(),
    );
    let resolved: ResolvedTenant = stored.into();

    assert_eq!(resolved.id, "550e8400-e29b-41d4-a716-446655440000");
}

#[test]
fn test_resolved_tenant_with_short_id() {
    let stored = StoredTenant::new("t1".to_string(), "Short".to_string());
    let resolved: ResolvedTenant = stored.into();

    assert_eq!(resolved.id, "t1");
}

#[test]
fn test_resolved_tenant_with_special_chars_in_name() {
    let stored = StoredTenant::new(
        "t-special".to_string(),
        "Tenant (Dev) - Test & Production".to_string(),
    );
    let resolved: ResolvedTenant = stored.into();

    assert_eq!(resolved.name, "Tenant (Dev) - Test & Production");
}
