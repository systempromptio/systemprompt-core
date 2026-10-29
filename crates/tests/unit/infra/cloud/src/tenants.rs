//! Unit tests for tenant storage types

use chrono::{TimeDelta, Utc};
use systemprompt_cloud::tenants::NewCloudTenantParams;
use systemprompt_cloud::{CloudTenantInfo, StoredTenant, TenantStore};
use systemprompt_identifiers::{CloudAppId, TenantId};
use systemprompt_manifest::HostingProvider;

#[test]
fn a_record_without_a_provider_is_a_local_docker_tenant() {
    let tenant: StoredTenant = serde_json::from_str(r#"{"id":"t1","name":"Legacy"}"#).unwrap();
    assert_eq!(tenant.provider, HostingProvider::DockerLocal);
}

#[test]
fn a_legacy_cloud_tenant_type_reads_as_the_systemprompt_provider() {
    let tenant: StoredTenant =
        serde_json::from_str(r#"{"id":"t1","name":"Astound","tenant_type":"cloud"}"#).unwrap();
    assert_eq!(tenant.provider, HostingProvider::Systemprompt);
    assert!(tenant.is_systemprompt_managed());
}

#[test]
fn a_legacy_local_tenant_type_reads_as_docker_local() {
    let tenant: StoredTenant =
        serde_json::from_str(r#"{"id":"t1","name":"Dev","tenant_type":"local"}"#).unwrap();
    assert_eq!(tenant.provider, HostingProvider::DockerLocal);
    assert!(tenant.is_local());
}

#[test]
fn a_legacy_record_is_rewritten_with_provider_on_save() {
    let tenant: StoredTenant =
        serde_json::from_str(r#"{"id":"t1","name":"Astound","tenant_type":"cloud"}"#).unwrap();
    let json = serde_json::to_string(&tenant).unwrap();
    assert!(json.contains("\"provider\":\"systemprompt\""), "{json}");
    assert!(!json.contains("tenant_type"), "{json}");
}

#[test]
fn a_legacy_tenants_file_loads_and_saves_in_the_new_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tenants.json");
    std::fs::write(
        &path,
        r#"{"tenants":[
            {"id":"t-cloud","name":"Prod","app_id":"sp-prod","hostname":"prod.example.com","region":"lhr","internal_database_url":"postgres://i/db","tenant_type":"cloud","external_db_access":false},
            {"id":"t-local","name":"Dev","database_url":"postgres://l/db","tenant_type":"local"}
        ],"synced_at":"2026-10-01T00:00:00Z"}"#,
    )
    .unwrap();

    let store = TenantStore::load_from_path(&path).unwrap();
    assert_eq!(store.tenants[0].provider, HostingProvider::Systemprompt);
    assert_eq!(store.tenants[1].provider, HostingProvider::DockerLocal);

    store.save_to_path(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(!saved.contains("tenant_type"), "{saved}");
    let value: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(value["tenants"][0]["provider"], "systemprompt");
    assert_eq!(value["tenants"][1]["provider"], "docker_local");
}

#[test]
fn an_unknown_provider_is_refused() {
    let err = serde_json::from_str::<StoredTenant>(r#"{"id":"t1","name":"X","provider":"heroku"}"#)
        .unwrap_err();
    assert!(err.to_string().contains("heroku"), "{err}");
}

#[test]
fn every_provider_round_trips_through_the_store_format() {
    for provider in HostingProvider::ALL {
        let mut tenant = StoredTenant::new(TenantId::new("t1"), "X".to_owned());
        tenant.provider = provider;
        let json = serde_json::to_string(&tenant).unwrap();
        let back: StoredTenant = serde_json::from_str(&json).unwrap();
        assert_eq!(back.provider, provider);
    }
}

#[test]
fn test_stored_tenant_new() {
    let tenant = StoredTenant::new(TenantId::new("tenant-123"), "My Tenant".to_string());

    assert_eq!(tenant.id.as_str(), "tenant-123");
    assert_eq!(tenant.name, "My Tenant");
    assert!(tenant.app_id.is_none());
    assert!(tenant.hostname.is_none());
    assert!(tenant.region.is_none());
    assert!(tenant.database_url.is_none());
    assert_eq!(tenant.provider, HostingProvider::DockerLocal);
}

#[test]
fn test_stored_tenant_new_with_empty_id() {
    let tenant = StoredTenant::new(TenantId::new(""), "Name".to_string());
    assert_eq!(tenant.id.as_str(), "");
}

#[test]
fn test_stored_tenant_new_with_empty_name() {
    let tenant = StoredTenant::new(TenantId::new("id"), "".to_string());
    assert_eq!(tenant.name, "");
}

#[test]
fn test_stored_tenant_new_local() {
    let tenant = StoredTenant::new_local(
        TenantId::new("local-123"),
        "Local Dev".to_string(),
        "postgres://localhost/dev".to_string(),
    );

    assert_eq!(tenant.id.as_str(), "local-123");
    assert_eq!(tenant.name, "Local Dev");
    assert_eq!(
        tenant.database_url,
        Some("postgres://localhost/dev".to_string())
    );
    assert_eq!(tenant.provider, HostingProvider::DockerLocal);
    assert!(tenant.app_id.is_none());
}

#[test]
fn test_stored_tenant_new_cloud() {
    let params = NewCloudTenantParams {
        id: TenantId::new("cloud-123"),
        name: "Production".to_string(),
        app_id: Some(CloudAppId::new("app-456")),
        hostname: Some("prod.systemprompt.io".to_string()),
        region: Some("iad".to_string()),
        database_url: Some("postgres://cloud/prod".to_string()),
        internal_database_url: "postgres://internal/prod".to_string(),
        external_db_access: false,
    };

    let tenant = StoredTenant::new_cloud(params);

    assert_eq!(tenant.id.as_str(), "cloud-123");
    assert_eq!(tenant.name, "Production");
    assert_eq!(tenant.app_id, Some(CloudAppId::new("app-456")));
    assert_eq!(tenant.hostname, Some("prod.systemprompt.io".to_string()));
    assert_eq!(tenant.region, Some("iad".to_string()));
    assert_eq!(tenant.provider, HostingProvider::Systemprompt);
}

#[test]
fn test_stored_tenant_new_cloud_minimal() {
    let params = NewCloudTenantParams {
        id: TenantId::new("cloud-minimal"),
        name: "Minimal".to_string(),
        app_id: None,
        hostname: None,
        region: None,
        database_url: None,
        internal_database_url: "postgres://minimal".to_string(),
        external_db_access: false,
    };

    let tenant = StoredTenant::new_cloud(params);

    assert!(tenant.app_id.is_none());
    assert!(tenant.hostname.is_none());
    assert!(tenant.region.is_none());
    assert_eq!(tenant.provider, HostingProvider::Systemprompt);
}

#[test]
fn test_stored_tenant_from_tenant_info() {
    let info = CloudTenantInfo {
        id: TenantId::new("info-123"),
        name: "From Info".to_string(),
        subscription_id: Some("sub-456".to_string()),
        subscription_status: None,
        app_id: Some(CloudAppId::new("app-789")),
        hostname: Some("info.systemprompt.io".to_string()),
        region: Some("lhr".to_string()),
        plan: None,
        status: None,
        external_db_access: false,
        database_url: "postgres://info".to_string(),
    };

    let tenant = StoredTenant::from_tenant_info(&info);

    assert_eq!(tenant.id.as_str(), "info-123");
    assert_eq!(tenant.name, "From Info");
    assert_eq!(tenant.app_id, Some(CloudAppId::new("app-789")));
    assert_eq!(tenant.hostname, Some("info.systemprompt.io".to_string()));
    assert_eq!(tenant.region, Some("lhr".to_string()));
    assert_eq!(tenant.provider, HostingProvider::Systemprompt);
}

#[test]
fn test_stored_tenant_from_tenant_info_minimal() {
    let info = CloudTenantInfo {
        id: TenantId::new("minimal"),
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

    assert_eq!(tenant.id.as_str(), "minimal");
    assert!(tenant.app_id.is_none());
    assert!(tenant.hostname.is_none());
    assert!(tenant.region.is_none());
}

#[test]
fn test_stored_tenant_has_database_url_true() {
    let tenant = StoredTenant::new_local(
        TenantId::new("id"),
        "name".to_string(),
        "postgres://localhost".to_string(),
    );
    assert!(tenant.has_database_url());
}

#[test]
fn test_stored_tenant_has_database_url_false_none() {
    let tenant = StoredTenant::new(TenantId::new("id"), "name".to_string());
    assert!(!tenant.has_database_url());
}

#[test]
fn test_stored_tenant_has_database_url_false_empty() {
    let mut tenant = StoredTenant::new(TenantId::new("id"), "name".to_string());
    tenant.database_url = Some("".to_string());
    assert!(!tenant.has_database_url());
}

#[test]
fn test_stored_tenant_serialization() {
    let tenant = StoredTenant::new(TenantId::new("ser-123"), "Serialize Me".to_string());

    let json = serde_json::to_string(&tenant).unwrap();
    assert!(json.contains("\"id\":\"ser-123\""));
    assert!(json.contains("\"name\":\"Serialize Me\""));
    assert!(json.contains("\"provider\":\"docker_local\""));
}

#[test]
fn test_stored_tenant_serialization_skips_none() {
    let tenant = StoredTenant::new(TenantId::new("id"), "name".to_string());

    let json = serde_json::to_string(&tenant).unwrap();
    assert!(!json.contains("\"app_id\""));
    assert!(!json.contains("\"hostname\""));
    assert!(!json.contains("\"region\""));
    assert!(!json.contains("\"database_url\""));
}

#[test]
fn test_tenant_store_new() {
    let tenants = vec![
        StoredTenant::new(TenantId::new("t1"), "Tenant 1".to_string()),
        StoredTenant::new(TenantId::new("t2"), "Tenant 2".to_string()),
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
        CloudTenantInfo {
            id: TenantId::new("i1"),
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
        CloudTenantInfo {
            id: TenantId::new("i2"),
            name: "Info 2".to_string(),
            subscription_id: None,
            subscription_status: None,
            app_id: Some(CloudAppId::new("app")),
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
    store
        .find_tenant(&TenantId::new("i1"))
        .expect("store.find_tenant(\"i1\") should be present");
    store
        .find_tenant(&TenantId::new("i2"))
        .expect("store.find_tenant(\"i2\") should be present");
}

#[test]
fn test_tenant_store_find_tenant_found() {
    let tenants = vec![
        StoredTenant::new(TenantId::new("find-me"), "Find Me".to_string()),
        StoredTenant::new(TenantId::new("other"), "Other".to_string()),
    ];
    let store = TenantStore::new(tenants);

    let found = store.find_tenant(&TenantId::new("find-me"));
    found.as_ref().expect("found should be present");
    assert_eq!(found.unwrap().name, "Find Me");
}

#[test]
fn test_tenant_store_find_tenant_not_found() {
    let tenants = vec![StoredTenant::new(
        TenantId::new("exists"),
        "Exists".to_string(),
    )];
    let store = TenantStore::new(tenants);

    let found = store.find_tenant(&TenantId::new("does-not-exist"));
    assert!(found.is_none());
}

#[test]
fn test_tenant_store_find_tenant_empty_store() {
    let store = TenantStore::new(vec![]);

    let found = store.find_tenant(&TenantId::new("any"));
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
    let tenants = vec![StoredTenant::new(
        TenantId::new("t1"),
        "Tenant 1".to_string(),
    )];
    let store = TenantStore::new(tenants);

    let json = serde_json::to_string(&store).unwrap();
    assert!(json.contains("\"tenants\""));
    assert!(json.contains("\"synced_at\""));
    assert!(json.contains("\"id\":\"t1\""));
}

fn control_plane_info(id: &str, name: &str) -> CloudTenantInfo {
    CloudTenantInfo {
        id: TenantId::new(id),
        name: name.to_string(),
        subscription_id: None,
        subscription_status: None,
        app_id: None,
        hostname: Some(format!("{id}.systemprompt.io")),
        region: None,
        plan: None,
        status: None,
        external_db_access: false,
        database_url: format!("postgres://{id}"),
    }
}

fn self_managed(id: &str, provider: HostingProvider) -> StoredTenant {
    StoredTenant::new_self_managed(systemprompt_cloud::NewSelfManagedTenantParams {
        id: TenantId::new(id),
        name: "Acme".to_owned(),
        provider,
        hostname: "ai.acme.example".to_owned(),
        region: Some("europe-west1".to_owned()),
        database_url: None,
    })
}

#[test]
fn control_plane_sync_never_touches_a_self_managed_record() {
    let mut store = TenantStore::new(vec![self_managed("acme", HostingProvider::Kubernetes)]);

    store.merge_control_plane(&[control_plane_info("acme", "Renamed by control plane")]);

    let tenant = store.find_tenant(&TenantId::new("acme")).unwrap();
    assert_eq!(tenant.provider, HostingProvider::Kubernetes);
    assert_eq!(tenant.name, "Acme");
    assert_eq!(tenant.hostname.as_deref(), Some("ai.acme.example"));
    assert_eq!(store.len(), 1);
}

#[test]
fn control_plane_sync_drops_systemprompt_tenants_the_account_no_longer_lists() {
    let gone = StoredTenant::from_tenant_info(&control_plane_info("sp-gone", "Gone"));
    let mut store = TenantStore::new(vec![gone, self_managed("k8s-1", HostingProvider::Gke)]);

    store.merge_control_plane(&[control_plane_info("sp-1", "Listed")]);

    assert_eq!(store.len(), 2);
    assert!(store.find_tenant(&TenantId::new("sp-gone")).is_none());
    assert!(store.find_tenant(&TenantId::new("sp-1")).is_some());
    assert!(store.find_tenant(&TenantId::new("k8s-1")).is_some());
}

#[test]
fn control_plane_sync_updates_and_adds_systemprompt_tenants_beside_self_managed_ones() {
    let mut existing = StoredTenant::from_tenant_info(&control_plane_info("sp-1", "Old"));
    existing.name = "Old".to_owned();
    let mut store = TenantStore::new(vec![existing, self_managed("k8s-1", HostingProvider::Gke)]);

    store.merge_control_plane(&[
        control_plane_info("sp-1", "New"),
        control_plane_info("sp-2", "Second"),
    ]);

    assert_eq!(store.len(), 3);
    assert_eq!(
        store.find_tenant(&TenantId::new("sp-1")).unwrap().name,
        "New"
    );
    assert_eq!(
        store.find_tenant(&TenantId::new("sp-2")).unwrap().provider,
        HostingProvider::Systemprompt
    );
    assert_eq!(
        store.find_tenant(&TenantId::new("k8s-1")).unwrap().provider,
        HostingProvider::Gke
    );
}

#[test]
fn insert_self_managed_refuses_systemprompt_and_duplicate_ids() {
    let mut store = TenantStore::default();
    store
        .insert_self_managed(self_managed("acme", HostingProvider::Kubernetes))
        .unwrap();

    let duplicate = store.insert_self_managed(self_managed("acme", HostingProvider::Gke));
    assert!(matches!(
        duplicate,
        Err(systemprompt_cloud::CloudError::TenantAlreadyExists { .. })
    ));

    let managed = store.insert_self_managed(self_managed("sp", HostingProvider::Systemprompt));
    assert!(matches!(
        managed,
        Err(systemprompt_cloud::CloudError::TenantNotSelfManaged { .. })
    ));
}

#[test]
fn a_self_managed_tenant_reads_its_database_url_not_the_internal_one() {
    let mut tenant = self_managed("acme", HostingProvider::Kubernetes);
    assert!(!tenant.has_database_url());
    tenant.database_url = Some("postgres://acme/db".to_owned());
    assert!(tenant.has_database_url());
    assert!(!tenant.has_missing_credentials());
}
