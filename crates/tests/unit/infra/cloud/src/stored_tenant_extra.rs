//! Additional unit tests for `StoredTenant` methods not exercised in
//! `tenants.rs`: `update_from_tenant_info`, `is_database_url_masked`,
//! `has_missing_credentials`, `uses_shared_container`,
//! `get_local_database_url`, and `new_local_shared`.

use systemprompt_cloud::{StoredTenant, TenantInfo, TenantType};

fn make_tenant_info(id: &str, db_url: &str) -> TenantInfo {
    TenantInfo {
        id: id.to_string(),
        name: "Test Tenant".to_string(),
        subscription_id: None,
        subscription_status: None,
        app_id: None,
        hostname: None,
        region: None,
        plan: None,
        status: None,
        external_db_access: false,
        database_url: db_url.to_string(),
    }
}

#[test]
fn new_local_shared_sets_correct_fields() {
    let tenant = StoredTenant::new_local_shared(
        "local-sh".to_string(),
        "Shared Tenant".to_string(),
        "postgres://localhost/shared".to_string(),
        "systemprompt-postgres-local".to_string(),
    );

    assert_eq!(tenant.id, "local-sh");
    assert_eq!(tenant.name, "Shared Tenant");
    assert_eq!(
        tenant.database_url,
        Some("postgres://localhost/shared".to_string())
    );
    assert_eq!(
        tenant.shared_container_db,
        Some("systemprompt-postgres-local".to_string())
    );
    assert_eq!(tenant.tenant_type, TenantType::Local);
    assert!(tenant.app_id.is_none());
    assert!(!tenant.external_db_access);
}

#[test]
fn uses_shared_container_true_when_set() {
    let tenant = StoredTenant::new_local_shared(
        "id".to_string(),
        "name".to_string(),
        "postgres://x".to_string(),
        "container".to_string(),
    );
    assert!(tenant.uses_shared_container());
}

#[test]
fn uses_shared_container_false_when_none() {
    let tenant = StoredTenant::new_local(
        "id".to_string(),
        "name".to_string(),
        "postgres://x".to_string(),
    );
    assert!(!tenant.uses_shared_container());
}

#[test]
fn uses_shared_container_false_for_cloud_tenant() {
    let tenant = StoredTenant::new("id".to_string(), "name".to_string());
    assert!(!tenant.uses_shared_container());
}

#[test]
fn get_local_database_url_returns_database_url_when_set() {
    let tenant = StoredTenant::new_local(
        "id".to_string(),
        "name".to_string(),
        "postgres://primary".to_string(),
    );
    let url = tenant.get_local_database_url();
    assert_eq!(url, Some(&"postgres://primary".to_string()));
}

#[test]
fn get_local_database_url_falls_back_to_internal() {
    let mut tenant = StoredTenant::new("id".to_string(), "name".to_string());
    tenant.internal_database_url = Some("postgres://internal".to_string());
    let url = tenant.get_local_database_url();
    assert_eq!(url, Some(&"postgres://internal".to_string()));
}

#[test]
fn get_local_database_url_returns_none_when_both_absent() {
    let tenant = StoredTenant::new("id".to_string(), "name".to_string());
    assert!(tenant.get_local_database_url().is_none());
}

#[test]
fn get_local_database_url_prefers_database_url_over_internal() {
    let mut tenant = StoredTenant::new_local(
        "id".to_string(),
        "name".to_string(),
        "postgres://primary".to_string(),
    );
    tenant.internal_database_url = Some("postgres://internal".to_string());
    let url = tenant.get_local_database_url();
    assert_eq!(url, Some(&"postgres://primary".to_string()));
}

#[test]
fn update_from_tenant_info_updates_name() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Old Name".to_string());
    let info = make_tenant_info("t-1", "postgres://new-db");
    tenant.update_from_tenant_info(&TenantInfo {
        name: "New Name".to_string(),
        ..info
    });
    assert_eq!(tenant.name, "New Name");
}

#[test]
fn update_from_tenant_info_updates_app_id() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    let info = TenantInfo {
        id: "t-1".to_string(),
        name: "Name".to_string(),
        subscription_id: None,
        subscription_status: None,
        app_id: Some("app-updated".to_string()),
        hostname: None,
        region: None,
        plan: None,
        status: None,
        external_db_access: false,
        database_url: "postgres://x".to_string(),
    };
    tenant.update_from_tenant_info(&info);
    assert_eq!(tenant.app_id, Some("app-updated".to_string()));
}

#[test]
fn update_from_tenant_info_updates_hostname() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    let info = TenantInfo {
        id: "t-1".to_string(),
        name: "Name".to_string(),
        subscription_id: None,
        subscription_status: None,
        app_id: None,
        hostname: Some("new.host.io".to_string()),
        region: None,
        plan: None,
        status: None,
        external_db_access: false,
        database_url: "postgres://x".to_string(),
    };
    tenant.update_from_tenant_info(&info);
    assert_eq!(tenant.hostname, Some("new.host.io".to_string()));
}

#[test]
fn update_from_tenant_info_updates_region() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    let info = TenantInfo {
        id: "t-1".to_string(),
        name: "Name".to_string(),
        subscription_id: None,
        subscription_status: None,
        app_id: None,
        hostname: None,
        region: Some("fra".to_string()),
        plan: None,
        status: None,
        external_db_access: false,
        database_url: "postgres://x".to_string(),
    };
    tenant.update_from_tenant_info(&info);
    assert_eq!(tenant.region, Some("fra".to_string()));
}

#[test]
fn update_from_tenant_info_updates_external_db_access() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    let info = TenantInfo {
        id: "t-1".to_string(),
        name: "Name".to_string(),
        subscription_id: None,
        subscription_status: None,
        app_id: None,
        hostname: None,
        region: None,
        plan: None,
        status: None,
        external_db_access: true,
        database_url: "postgres://x".to_string(),
    };
    tenant.update_from_tenant_info(&info);
    assert!(tenant.external_db_access);
}

#[test]
fn update_from_tenant_info_sets_internal_database_url_when_not_masked() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    let info = make_tenant_info("t-1", "postgres://user:realpass@host/db");
    tenant.update_from_tenant_info(&info);
    assert_eq!(
        tenant.internal_database_url,
        Some("postgres://user:realpass@host/db".to_string())
    );
}

#[test]
fn update_from_tenant_info_skips_internal_url_when_masked_with_stars() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.internal_database_url = Some("postgres://user:realpass@host/db".to_string());
    let info = make_tenant_info("t-1", "postgres://user:***@host/db");
    tenant.update_from_tenant_info(&info);
    assert_eq!(
        tenant.internal_database_url,
        Some("postgres://user:realpass@host/db".to_string())
    );
}

#[test]
fn update_from_tenant_info_updates_internal_url_when_masked_only_by_many_stars() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.internal_database_url = Some("postgres://user:secret@host/db".to_string());
    let info = make_tenant_info("t-1", "postgres://user:********@host/db");
    tenant.update_from_tenant_info(&info);
    assert_eq!(
        tenant.internal_database_url,
        Some("postgres://user:********@host/db".to_string())
    );
}

#[test]
fn is_database_url_masked_true_for_three_star_mask() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.internal_database_url = Some("postgres://user:***@host/db".to_string());
    assert!(tenant.is_database_url_masked());
}

#[test]
fn is_database_url_masked_true_for_eight_star_mask() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.internal_database_url = Some("postgres://user:********@host/db".to_string());
    assert!(tenant.is_database_url_masked());
}

#[test]
fn is_database_url_masked_false_for_real_password() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.internal_database_url = Some("postgres://user:realpass@host/db".to_string());
    assert!(!tenant.is_database_url_masked());
}

#[test]
fn is_database_url_masked_false_when_no_internal_url() {
    let tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    assert!(!tenant.is_database_url_masked());
}

#[test]
fn has_missing_credentials_true_for_cloud_tenant_with_masked_url() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.tenant_type = TenantType::Cloud;
    tenant.internal_database_url = Some("postgres://user:***@host/db".to_string());
    assert!(tenant.has_missing_credentials());
}

#[test]
fn has_missing_credentials_false_for_cloud_tenant_with_real_url() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.tenant_type = TenantType::Cloud;
    tenant.internal_database_url = Some("postgres://user:realpass@host/db".to_string());
    assert!(!tenant.has_missing_credentials());
}

#[test]
fn has_missing_credentials_false_for_local_tenant_even_with_masked_url() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.tenant_type = TenantType::Local;
    tenant.internal_database_url = Some("postgres://user:***@host/db".to_string());
    assert!(!tenant.has_missing_credentials());
}

#[test]
fn has_missing_credentials_false_when_no_internal_url() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.tenant_type = TenantType::Cloud;
    assert!(!tenant.has_missing_credentials());
}

#[test]
fn is_cloud_true_for_cloud_tenant() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.tenant_type = TenantType::Cloud;
    assert!(tenant.is_cloud());
    assert!(!tenant.is_local());
}

#[test]
fn is_local_true_for_local_tenant() {
    let tenant = StoredTenant::new_local(
        "t-1".to_string(),
        "Name".to_string(),
        "postgres://x".to_string(),
    );
    assert!(tenant.is_local());
    assert!(!tenant.is_cloud());
}

#[test]
fn has_database_url_cloud_true_when_internal_url_set() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.tenant_type = TenantType::Cloud;
    tenant.internal_database_url = Some("postgres://internal".to_string());
    assert!(tenant.has_database_url());
}

#[test]
fn has_database_url_cloud_false_when_internal_url_empty_string() {
    let mut tenant = StoredTenant::new("t-1".to_string(), "Name".to_string());
    tenant.tenant_type = TenantType::Cloud;
    tenant.internal_database_url = Some(String::new());
    assert!(!tenant.has_database_url());
}
