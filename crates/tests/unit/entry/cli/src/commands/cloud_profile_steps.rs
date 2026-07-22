//! Tests for `cloud profile` creation steps: tenant resolution from args,
//! secrets writing, and masked-credential refresh gating.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::profile_steps::{
    ensure_unmasked_credentials, resolve_tenant_from_args, write_profile_secrets,
};
use systemprompt_cli::cloud::profile::{ApiKeys, CreateArgs, TenantTypeArg};
use systemprompt_cloud::{StoredTenant, TenantStore, TenantType};
use systemprompt_identifiers::TenantId;

fn create_args(tenant: Option<&str>, tenant_type: TenantTypeArg) -> CreateArgs {
    CreateArgs {
        name: "steps-test".to_owned(),
        tenant: tenant.map(str::to_owned),
        tenant_type,
        anthropic_key: None,
        openai_key: None,
        gemini_key: None,
        github_token: None,
    }
}

fn local_tenant(id: &str) -> StoredTenant {
    StoredTenant::new_local(
        TenantId::new(id),
        format!("{id}-name"),
        "postgres://user:pw@localhost:5432/steps".to_owned(),
    )
}

fn store_with(tenant: StoredTenant) -> TenantStore {
    let mut store = TenantStore::default();
    store.tenants.push(tenant);
    store
}

fn api_keys() -> ApiKeys {
    ApiKeys {
        gemini: None,
        anthropic: Some("ant-key".to_owned()),
        openai: None,
    }
}

#[test]
fn resolve_tenant_from_args_requires_tenant_id() {
    let err = resolve_tenant_from_args(
        &create_args(None, TenantTypeArg::Local),
        &TenantStore::default(),
    )
    .expect_err("missing tenant id");
    assert!(err.to_string().contains("--tenant-id"), "{err}");
}

#[test]
fn resolve_tenant_from_args_rejects_unknown_tenant() {
    let err = resolve_tenant_from_args(
        &create_args(Some("absent"), TenantTypeArg::Local),
        &store_with(local_tenant("present")),
    )
    .expect_err("unknown tenant");
    assert!(err.to_string().contains("'absent' not found"), "{err}");
}

#[test]
fn resolve_tenant_from_args_rejects_type_mismatch() {
    let err = resolve_tenant_from_args(
        &create_args(Some("t1"), TenantTypeArg::Cloud),
        &store_with(local_tenant("t1")),
    )
    .expect_err("type mismatch");
    assert!(err.to_string().contains("is type Local"), "{err}");
}

#[test]
fn resolve_tenant_from_args_returns_matching_tenant() {
    let tenant = resolve_tenant_from_args(
        &create_args(Some("t1"), TenantTypeArg::Local),
        &store_with(local_tenant("t1")),
    )
    .expect("tenant resolved");
    assert_eq!(tenant.id.as_str(), "t1");
    assert_eq!(tenant.tenant_type, TenantType::Local);
}

#[test]
fn write_profile_secrets_writes_database_url_and_key() {
    let dir = tempfile::tempdir().unwrap();
    write_profile_secrets(&local_tenant("t1"), &api_keys(), dir.path()).expect("secrets written");

    let secrets_path = dir.path().join("secrets.json");
    let raw = std::fs::read_to_string(&secrets_path).expect("secrets.json exists");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("valid json");
    let flattened = json.to_string();
    assert!(
        flattened.contains("postgres://user:pw@localhost:5432/steps"),
        "{flattened}"
    );
    assert!(flattened.contains("ant-key"), "{flattened}");
}

#[test]
fn write_profile_secrets_requires_database_url() {
    let dir = tempfile::tempdir().unwrap();
    let tenant = StoredTenant::new(TenantId::new("nourl"), "nourl".to_owned());
    let err = write_profile_secrets(&tenant, &api_keys(), dir.path()).expect_err("no db url");
    assert!(
        err.to_string().contains("database URL is required"),
        "{err}"
    );
}

#[tokio::test]
async fn ensure_unmasked_credentials_passes_local_tenant_through() {
    let dir = tempfile::tempdir().unwrap();
    let tenants_path = dir.path().join("tenants.json");
    let tenant = ensure_unmasked_credentials(local_tenant("t1"), &tenants_path)
        .await
        .expect("local tenant unchanged");
    assert_eq!(tenant.id.as_str(), "t1");
    assert!(!tenants_path.exists(), "store must not be touched");
}

#[tokio::test]
async fn ensure_unmasked_credentials_skips_refresh_for_unmasked_cloud_tenant() {
    let dir = tempfile::tempdir().unwrap();
    let tenants_path = dir.path().join("tenants.json");

    let mut tenant = StoredTenant::new(TenantId::new("c1"), "c1".to_owned());
    tenant.tenant_type = TenantType::Cloud;
    tenant.external_db_access = true;
    tenant.database_url = Some("postgres://user:realpw@db.example:5432/app".to_owned());
    tenant.internal_database_url = Some("postgres://user:realpw@internal:5432/app".to_owned());

    let result = ensure_unmasked_credentials(tenant, &tenants_path)
        .await
        .expect("no refresh needed");
    assert_eq!(
        result.database_url.as_deref(),
        Some("postgres://user:realpw@db.example:5432/app")
    );
    assert!(!tenants_path.exists(), "store must not be touched");
}
