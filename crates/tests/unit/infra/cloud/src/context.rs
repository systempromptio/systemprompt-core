//! Unit tests for `ResolvedTenant` and the pure-logic accessors on
//! `CloudContext`.

use systemprompt_cloud::context::{CloudContext, ResolvedTenant};
use systemprompt_cloud::{StoredTenant, TenantType};

fn make_resolved(id: &str, name: &str) -> ResolvedTenant {
    ResolvedTenant {
        id: id.to_string(),
        name: name.to_string(),
        app_id: None,
        hostname: None,
        region: None,
    }
}

fn make_resolved_full(
    id: &str,
    name: &str,
    app_id: &str,
    hostname: &str,
    region: &str,
) -> ResolvedTenant {
    ResolvedTenant {
        id: id.to_string(),
        name: name.to_string(),
        app_id: Some(app_id.to_string()),
        hostname: Some(hostname.to_string()),
        region: Some(region.to_string()),
    }
}

#[test]
fn resolved_tenant_debug_contains_id() {
    let rt = make_resolved("t-001", "My Tenant");
    let d = format!("{rt:?}");
    assert!(d.contains("t-001"));
    assert!(d.contains("ResolvedTenant"));
}

#[test]
fn resolved_tenant_clone_equals_original() {
    let rt = make_resolved_full("t-001", "My Tenant", "app-1", "example.com", "iad");
    let cloned = rt.clone();
    assert_eq!(cloned.id, rt.id);
    assert_eq!(cloned.name, rt.name);
    assert_eq!(cloned.app_id, rt.app_id);
    assert_eq!(cloned.hostname, rt.hostname);
    assert_eq!(cloned.region, rt.region);
}

#[test]
fn resolved_tenant_from_stored_tenant_maps_fields() {
    let stored = StoredTenant {
        id: "stored-1".to_string(),
        name: "Stored Name".to_string(),
        app_id: Some("app-stored".to_string()),
        hostname: Some("stored.example.com".to_string()),
        region: Some("lhr".to_string()),
        database_url: None,
        internal_database_url: None,
        tenant_type: TenantType::Cloud,
        external_db_access: false,
        shared_container_db: None,
    };

    let resolved = ResolvedTenant::from(stored);

    assert_eq!(resolved.id, "stored-1");
    assert_eq!(resolved.name, "Stored Name");
    assert_eq!(resolved.app_id, Some("app-stored".to_string()));
    assert_eq!(resolved.hostname, Some("stored.example.com".to_string()));
    assert_eq!(resolved.region, Some("lhr".to_string()));
}

#[test]
fn resolved_tenant_from_stored_tenant_preserves_none_optionals() {
    let stored = StoredTenant {
        id: "bare-id".to_string(),
        name: "Bare Name".to_string(),
        app_id: None,
        hostname: None,
        region: None,
        database_url: None,
        internal_database_url: None,
        tenant_type: TenantType::Local,
        external_db_access: false,
        shared_container_db: None,
    };

    let resolved = ResolvedTenant::from(stored);

    assert_eq!(resolved.id, "bare-id");
    assert!(resolved.app_id.is_none());
    assert!(resolved.hostname.is_none());
    assert!(resolved.region.is_none());
}

#[test]
fn cloud_context_has_tenant_true_when_tenant_set() {
    let rt = make_resolved("t-x", "X");
    let ctx = make_context_with_tenant(Some(rt));
    assert!(ctx.has_tenant());
}

#[test]
fn cloud_context_has_tenant_false_when_no_tenant() {
    let ctx = make_context_with_tenant(None);
    assert!(!ctx.has_tenant());
}

#[test]
fn cloud_context_tenant_id_ok_when_tenant_present() {
    let rt = make_resolved("my-tenant-id", "My Name");
    let ctx = make_context_with_tenant(Some(rt));
    assert_eq!(ctx.tenant_id().unwrap(), "my-tenant-id");
}

#[test]
fn cloud_context_tenant_id_err_when_no_tenant() {
    let ctx = make_context_with_tenant(None);
    ctx.tenant_id().unwrap_err();
}

#[test]
fn cloud_context_app_id_ok_when_present() {
    let rt = make_resolved_full("t-1", "Name", "my-app-id", "h.io", "iad");
    let ctx = make_context_with_tenant(Some(rt));
    assert_eq!(ctx.app_id().unwrap(), "my-app-id");
}

#[test]
fn cloud_context_app_id_err_when_no_tenant() {
    let ctx = make_context_with_tenant(None);
    ctx.app_id().unwrap_err();
}

#[test]
fn cloud_context_app_id_err_when_tenant_has_no_app_id() {
    let rt = make_resolved("t-1", "Name");
    let ctx = make_context_with_tenant(Some(rt));
    ctx.app_id().unwrap_err();
}

#[test]
fn cloud_context_tenant_name_unknown_when_no_tenant() {
    let ctx = make_context_with_tenant(None);
    assert_eq!(ctx.tenant_name(), "unknown");
}

#[test]
fn cloud_context_tenant_name_returns_name() {
    let rt = make_resolved("t-1", "Prod Tenant");
    let ctx = make_context_with_tenant(Some(rt));
    assert_eq!(ctx.tenant_name(), "Prod Tenant");
}

#[test]
fn cloud_context_hostname_none_when_no_tenant() {
    let ctx = make_context_with_tenant(None);
    assert!(ctx.hostname().is_none());
}

#[test]
fn cloud_context_hostname_none_when_tenant_has_no_hostname() {
    let rt = make_resolved("t-1", "Name");
    let ctx = make_context_with_tenant(Some(rt));
    assert!(ctx.hostname().is_none());
}

#[test]
fn cloud_context_hostname_returns_hostname() {
    let rt = make_resolved_full("t-1", "Name", "app", "my.hostname.io", "sin");
    let ctx = make_context_with_tenant(Some(rt));
    assert_eq!(ctx.hostname(), Some("my.hostname.io"));
}

#[test]
fn cloud_context_profile_err_when_not_loaded() {
    let ctx = make_context_with_tenant(None);
    ctx.profile().unwrap_err();
}

fn make_context_with_tenant(tenant: Option<ResolvedTenant>) -> CloudContext {
    let creds = systemprompt_cloud::CloudCredentials::new(
        "dummy-token".to_string(),
        "https://api.systemprompt.io".to_string(),
        "test@example.com".to_string(),
    );
    let api_client =
        systemprompt_cloud::CloudApiClient::new("https://api.systemprompt.io", "dummy-token")
            .expect("client");
    CloudContext {
        credentials: creds,
        profile: None,
        tenant,
        api_client,
    }
}
