//! Harness tests for the cloud-reconciliation half of `cloud tenant list`:
//! a tenant that exists only in the cloud account is appended to the local
//! store, and an upstream failure leaves the local store intact.

use serde_json::json;
use systemprompt_cli::cloud::tenant::TenantCommands;
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{OTHER_TENANT_ID, TENANT_ID, USER_EMAIL, enter, json_ctx};

async fn run_tenant_list() {
    cloud::execute(
        CloudCommands::Tenant {
            command: Some(TenantCommands::List),
        },
        &json_ctx(),
    )
    .await
    .expect("tenant list");
}

fn stored_tenants() -> TenantStore {
    TenantStore::load_from_path(&get_cloud_paths().resolve(CloudPath::Tenants))
        .expect("tenant store on disk")
}

#[tokio::test]
async fn tenant_list_appends_cloud_only_tenant_to_local_store() {
    let env = enter().await;
    env.server().reset().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": { "id": "user_harness", "email": USER_EMAIL, "name": "Harness" },
            "tenants": [{
                "id": "t-cloud-only",
                "name": "Cloud Only",
                "hostname": "cloud-only.example.com",
                "region": "ams",
                "external_db_access": false,
                "database_url": "postgres://int/cloud-only"
            }]
        })))
        .mount(env.server())
        .await;

    run_tenant_list().await;

    let store = stored_tenants();
    let added = store
        .tenants
        .iter()
        .find(|t| t.id.as_str() == "t-cloud-only")
        .expect("cloud-only tenant appended to the local store");
    assert_eq!(added.name, "Cloud Only");
    assert_eq!(added.region.as_deref(), Some("ams"));
    assert!(
        store.tenants.iter().any(|t| t.id.as_str() == TENANT_ID),
        "sync must not drop pre-existing local entries"
    );
}

#[tokio::test]
async fn tenant_list_keeps_local_store_when_cloud_sync_fails() {
    let env = enter().await;
    env.server().reset().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    run_tenant_list().await;

    let store = stored_tenants();
    assert!(store.tenants.iter().any(|t| t.id.as_str() == TENANT_ID));
    assert!(
        store
            .tenants
            .iter()
            .any(|t| t.id.as_str() == OTHER_TENANT_ID)
    );
}
