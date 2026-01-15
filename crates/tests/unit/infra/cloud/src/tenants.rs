//! Unit tests for tenant storage types

use chrono::{TimeDelta, Utc};
use systemprompt_cloud::tenants::NewCloudTenantParams;
use systemprompt_cloud::{StoredTenant, TenantInfo, TenantStore, TenantType};

#[test]
fn test_tenant_type_default_is_local() {
    let default = TenantType::default();
    assert_eq!(default, TenantType::Local);
}

#[test]
fn test_tenant_type_variants() {
    let local = TenantType::Local;
    let cloud = TenantType::Cloud;

    assert_ne!(local, cloud);
}

#[test]
fn test_tenant_type_serialization() {
    let local = TenantType::Local;
    let json = serde_json::to_string(&local).unwrap();
    assert_eq!(json, "\"local\"");

    let cloud = TenantType::Cloud;
    let json = serde_json::to_string(&cloud).unwrap();
    assert_eq!(json, "\"cloud\"");
}

#[test]
fn test_tenant_type_deserialization() {
    let local: TenantType = serde_json::from_str("\"local\"").unwrap();
    assert_eq!(local, TenantType::Local);

    let cloud: TenantType = serde_json::from_str("\"cloud\"").unwrap();
    assert_eq!(cloud, TenantType::Cloud);
}

#[test]
fn test_stored_tenant_new() {
    let tenant = StoredTenant::new("tenant-123".to_string(), "My Tenant".to_string());

    assert_eq!(tenant.id, "tenant-123");
    assert_eq!(tenant.name, "My Tenant");
    assert!(tenant.app_id.is_none());
    assert!(tenant.hostname.is_none());
    assert!(tenant.region.is_none());
    assert!(tenant.database_url.is_none());
    assert_eq!(tenant.tenant_type, TenantType::Local);
}

#[test]
fn test_stored_tenant_new_with_empty_id() {
    let tenant = StoredTenant::new("".to_string(), "Name".to_string());
    assert_eq!(tenant.id, "");
}

#[test]
fn test_stored_tenant_new_with_empty_name() {
    let tenant = StoredTenant::new("id".to_string(), "".to_string());
    assert_eq!(tenant.name, "");
}

#[test]
fn test_stored_tenant_new_local() {
    let tenant = StoredTenant::new_local(
        "local-123".to_string(),
        "Local Dev".to_string(),
        "postgres://localhost/dev".to_string(),
    );

    assert_eq!(tenant.id, "local-123");
    assert_eq!(tenant.name, "Local Dev");
    assert_eq!(
        tenant.database_url,
        Some("postgres://localhost/dev".to_string())
    );
    assert_eq!(tenant.tenant_type, TenantType::Local);
    assert!(tenant.app_id.is_none());
}

#[test]
fn test_stored_tenant_new_cloud() {
    let params = NewCloudTenantParams {
        id: "cloud-123".to_string(),
        name: "Production".to_string(),
        app_id: Some("app-456".to_string()),
        hostname: Some("prod.systemprompt.io".to_string()),
        region: Some("iad".to_string()),
        database_url: Some("postgres://cloud/prod".to_string()),
        internal_database_url: "postgres://internal/prod".to_string(),
        external_db_access: false,
        sync_token: None,
    };

    let tenant = StoredTenant::new_cloud(params);

    assert_eq!(tenant.id, "cloud-123");
    assert_eq!(tenant.name, "Production");
    assert_eq!(tenant.app_id, Some("app-456".to_string()));
    assert_eq!(tenant.hostname, Some("prod.systemprompt.io".to_string()));
    assert_eq!(tenant.region, Some("iad".to_string()));
    assert_eq!(tenant.tenant_type, TenantType::Cloud);
}

#[test]
fn test_stored_tenant_new_cloud_minimal() {
    let params = NewCloudTenantParams {
        id: "cloud-minimal".to_string(),
        name: "Minimal".to_string(),
        app_id: None,
        hostname: None,
        region: None,
        database_url: None,
        internal_database_url: "postgres://minimal".to_string(),
        external_db_access: false,
        sync_token: None,
    };

    let tenant = StoredTenant::new_cloud(params);

    assert!(tenant.app_id.is_none());
    assert!(tenant.hostname.is_none());
    assert!(tenant.region.is_none());
    assert_eq!(tenant.tenant_type, TenantType::Cloud);
}

#[test]
fn test_stored_tenant_from_tenant_info() {
    let info = TenantInfo {
        id: "info-123".to_string(),
        name: "From Info".to_string(),
        subscription_id: Some("sub-456".to_string()),
        subscription_status: None,
        app_id: Some("app-789".to_string()),
        hostname: Some("info.systemprompt.io".to_string()),
        region: Some("lhr".to_string()),
        plan: None,
        status: None,
        external_db_access: false,
        database_url: "postgres://info".to_string(),
    };

    let tenant = StoredTenant::from_tenant_info(&info);

    assert_eq!(tenant.id, "info-123");
    assert_eq!(tenant.name, "From Info");
    assert_eq!(tenant.app_id, Some("app-789".to_string()));
    assert_eq!(tenant.hostname, Some("info.systemprompt.io".to_string()));
    assert_eq!(tenant.region, Some("lhr".to_string()));
    assert_eq!(tenant.tenant_type, TenantType::Cloud);
}

#[test]
fn test_stored_tenant_from_tenant_info_minimal() {
    let info = TenantInfo {
        id: "minimal".to_string(),
        name: "Minimal Info".to_string(),
        subscription_id: None,
        subscription_status: None,
        app_id: None,
        hostname: None,
        region: None,
        plan: None,
        status: None,
        external_db_access: false,
        database_url: "postgres://minimal".to_string(),
    };

    let tenant = StoredTenant::from_tenant_info(&info);

    assert_eq!(tenant.id, "minimal");
    assert!(tenant.app_id.is_none());
    assert!(tenant.hostname.is_none());
    assert!(tenant.region.is_none());
}

#[test]
fn test_stored_tenant_has_database_url_true() {
    let tenant = StoredTenant::new_local(
        "id".to_string(),
        "name".to_string(),
        "postgres://localhost".to_string(),
    );
    assert!(tenant.has_database_url());
}

#[test]
fn test_stored_tenant_has_database_url_false_none() {
    let tenant = StoredTenant::new("id".to_string(), "name".to_string());
    assert!(!tenant.has_database_url());
}

#[test]
fn test_stored_tenant_has_database_url_false_empty() {
    let mut tenant = StoredTenant::new("id".to_string(), "name".to_string());
    tenant.database_url = Some("".to_string());
    assert!(!tenant.has_database_url());
}

#[test]
fn test_stored_tenant_serialization() {
    let tenant = StoredTenant::new("ser-123".to_string(), "Serialize Me".to_string());

    let json = serde_json::to_string(&tenant).unwrap();
    assert!(json.contains("\"id\":\"ser-123\""));
    assert!(json.contains("\"name\":\"Serialize Me\""));
    assert!(json.contains("\"tenant_type\":\"local\""));
}

#[test]
fn test_stored_tenant_serialization_skips_none() {
    let tenant = StoredTenant::new("id".to_string(), "name".to_string());

    let json = serde_json::to_string(&tenant).unwrap();
    assert!(!json.contains("\"app_id\""));
    assert!(!json.contains("\"hostname\""));
    assert!(!json.contains("\"region\""));
    assert!(!json.contains("\"database_url\""));
}

#[test]
fn test_stored_tenant_deserialization() {
    let json = r#"{
        "id": "deser-123",
        "name": "Deserialized",
        "tenant_type": "cloud"
    }"#;

    let tenant: StoredTenant = serde_json::from_str(json).unwrap();
    assert_eq!(tenant.id, "deser-123");
    assert_eq!(tenant.name, "Deserialized");
    assert_eq!(tenant.tenant_type, TenantType::Cloud);
}

#[test]
fn test_stored_tenant_deserialization_with_optional_fields() {
    let json = r#"{
        "id": "full-123",
        "name": "Full Tenant",
        "app_id": "app-456",
        "hostname": "full.example.com",
        "region": "fra",
        "database_url": "postgres://full",
        "tenant_type": "cloud"
    }"#;

    let tenant: StoredTenant = serde_json::from_str(json).unwrap();
    assert_eq!(tenant.app_id, Some("app-456".to_string()));
    assert_eq!(tenant.hostname, Some("full.example.com".to_string()));
    assert_eq!(tenant.region, Some("fra".to_string()));
    assert_eq!(tenant.database_url, Some("postgres://full".to_string()));
}

#[test]
fn test_tenant_store_new() {
    let tenants = vec![
        StoredTenant::new("t1".to_string(), "Tenant 1".to_string()),
        StoredTenant::new("t2".to_string(), "Tenant 2".to_string()),
    ];

    let store = TenantStore::new(tenants);

    assert_eq!(store.len(), 2);
    assert!(!store.is_empty());
}

#[test]
fn test_tenant_store_new_empty() {
    let store = TenantStore::new(vec![]);

    assert_eq!(store.len(), 0);
    assert!(store.is_empty());
}

#[test]
fn test_tenant_store_synced_at() {
    let before = Utc::now();
    let store = TenantStore::new(vec![]);
    let after = Utc::now();

    assert!(store.synced_at >= before);
    assert!(store.synced_at <= after);
}

#[test]
fn test_tenant_store_from_tenant_infos() {
    let infos = vec![
        TenantInfo {
            id: "i1".to_string(),
            name: "Info 1".to_string(),
            subscription_id: None,
            subscription_status: None,
            app_id: None,
            hostname: None,
            region: None,
            plan: None,
            status: None,
            external_db_access: false,
            database_url: "postgres://i1".to_string(),
        },
        TenantInfo {
            id: "i2".to_string(),
            name: "Info 2".to_string(),
            subscription_id: None,
            subscription_status: None,
            app_id: Some("app".to_string()),
            hostname: None,
            region: None,
            plan: None,
            status: None,
            external_db_access: false,
            database_url: "postgres://i2".to_string(),
        },
    ];

    let store = TenantStore::from_tenant_infos(&infos);

    assert_eq!(store.len(), 2);
    assert!(store.find_tenant("i1").is_some());
    assert!(store.find_tenant("i2").is_some());
}

#[test]
fn test_tenant_store_find_tenant_found() {
    let tenants = vec![
        StoredTenant::new("find-me".to_string(), "Find Me".to_string()),
        StoredTenant::new("other".to_string(), "Other".to_string()),
    ];
    let store = TenantStore::new(tenants);

    let found = store.find_tenant("find-me");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "Find Me");
}

#[test]
fn test_tenant_store_find_tenant_not_found() {
    let tenants = vec![StoredTenant::new("exists".to_string(), "Exists".to_string())];
    let store = TenantStore::new(tenants);

    let found = store.find_tenant("does-not-exist");
    assert!(found.is_none());
}

#[test]
fn test_tenant_store_find_tenant_empty_store() {
    let store = TenantStore::new(vec![]);

    let found = store.find_tenant("any");
    assert!(found.is_none());
}

#[test]
fn test_tenant_store_is_stale_fresh() {
    let store = TenantStore::new(vec![]);
    assert!(!store.is_stale(TimeDelta::hours(1)));
}

#[test]
fn test_tenant_store_is_stale_old() {
    let mut store = TenantStore::new(vec![]);
    store.synced_at = Utc::now() - TimeDelta::hours(2);
    assert!(store.is_stale(TimeDelta::hours(1)));
}

#[test]
fn test_tenant_store_is_stale_at_boundary() {
    let mut store = TenantStore::new(vec![]);
    store.synced_at = Utc::now() - TimeDelta::minutes(59);
    assert!(!store.is_stale(TimeDelta::hours(1)));
}

#[test]
fn test_tenant_store_is_stale_with_days() {
    let mut store = TenantStore::new(vec![]);
    store.synced_at = Utc::now() - TimeDelta::days(7);

    assert!(store.is_stale(TimeDelta::days(1)));
    assert!(!store.is_stale(TimeDelta::days(30)));
}

#[test]
fn test_tenant_store_default() {
    let store = TenantStore::default();

    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_tenant_store_serialization() {
    let tenants = vec![StoredTenant::new("t1".to_string(), "Tenant 1".to_string())];
    let store = TenantStore::new(tenants);

    let json = serde_json::to_string(&store).unwrap();
    assert!(json.contains("\"tenants\""));
    assert!(json.contains("\"synced_at\""));
    assert!(json.contains("\"id\":\"t1\""));
}

#[test]
fn test_tenant_store_deserialization() {
    let json = r#"{
        "tenants": [
            {"id": "d1", "name": "Deserialized 1", "tenant_type": "local"},
            {"id": "d2", "name": "Deserialized 2", "tenant_type": "cloud"}
        ],
        "synced_at": "2024-01-15T12:00:00Z"
    }"#;

    let store: TenantStore = serde_json::from_str(json).unwrap();
    assert_eq!(store.len(), 2);
    assert!(store.find_tenant("d1").is_some());
    assert!(store.find_tenant("d2").is_some());
}

#[test]
fn test_tenant_store_roundtrip() {
    let tenants = vec![
        StoredTenant::new("rt1".to_string(), "Roundtrip 1".to_string()),
        StoredTenant::new_local(
            "rt2".to_string(),
            "Roundtrip 2".to_string(),
            "postgres://rt2".to_string(),
        ),
    ];
    let original = TenantStore::new(tenants);

    let json = serde_json::to_string(&original).unwrap();
    let restored: TenantStore = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.len(), original.len());
    assert_eq!(
        restored.find_tenant("rt1").unwrap().name,
        original.find_tenant("rt1").unwrap().name
    );
}
