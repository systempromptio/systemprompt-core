//! Wiremock tests for `TenantProvisioningService::finalize_tenant`: the
//! SSE-to-polling provisioning wait, credential retrieval, external-access
//! configuration, and `swap_to_external_host`.

use std::sync::Mutex;

use serde_json::json;
use systemprompt_cloud::CloudApiClient;
use systemprompt_cloud::tenants::{
    ProvisioningProgress, ProvisioningProgressEvent, TenantProvisioningService,
    swap_to_external_host,
};
use systemprompt_identifiers::TenantId;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct RecordingProgress {
    events: Mutex<Vec<String>>,
}

impl RecordingProgress {
    const fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    fn labels(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

impl ProvisioningProgress for RecordingProgress {
    fn event(&self, event: &ProvisioningProgressEvent<'_>) {
        let debug = format!("{event:?}");
        let label = debug
            .split([' ', '(', '{'])
            .next()
            .unwrap_or_default()
            .to_owned();
        self.events.lock().unwrap().push(label);
    }
}

async fn token_mock(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tenant_bearer",
            "expires_in": 600
        })))
        .mount(server)
        .await;
}

async fn mount_ready_tenant(server: &MockServer, tenant: &str, database_url: &str) {
    token_mock(server).await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{tenant}/events")))
        .respond_with(ResponseTemplate::new(404))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{tenant}/status")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "status": "ready",
                "secrets_url": format!("{}/api/v1/tenants/{tenant}/secrets-bundle", server.uri())
            }
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{tenant}/secrets-bundle")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "database_url": database_url,
            "internal_database_url": database_url,
            "app_url": "https://demo.fly.dev"
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": { "id": "user-1", "email": "dev@example.com" },
            "tenants": [{
                "id": tenant,
                "name": "demo",
                "app_id": "app-1",
                "hostname": "demo.fly.dev",
                "region": "lhr",
                "database_url": database_url
            }]
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn finalize_tenant_polls_to_ready_and_builds_record() {
    let server = MockServer::start().await;
    let db_url = "postgres://u:p@internal.systemprompt.io:5432/demo";
    mount_ready_tenant(&server, "t-prov", db_url).await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let progress = RecordingProgress::new();
    let tenant = TenantProvisioningService::new(&client)
        .finalize_tenant(&TenantId::new("t-prov"), false, &progress)
        .await
        .expect("finalize");

    assert_eq!(tenant.id, "t-prov");
    assert_eq!(tenant.name, "demo");
    assert_eq!(tenant.app_id.as_deref(), Some("app-1"));
    assert_eq!(tenant.hostname.as_deref(), Some("demo.fly.dev"));
    assert_eq!(tenant.internal_database_url.as_deref(), Some(db_url));
    assert_eq!(tenant.database_url, None);
    assert!(!tenant.external_db_access);
    assert!(tenant.is_cloud());

    assert_eq!(
        progress.labels(),
        vec![
            "ProvisioningStarted",
            "Provisioned",
            "CredentialsFetchStarted",
            "CredentialsFetched",
            "TenantSyncStarted",
            "TenantSynced",
        ]
    );
}

#[tokio::test]
async fn finalize_tenant_enables_external_access_and_swaps_host() {
    let server = MockServer::start().await;
    let db_url = "postgres://u:p@internal.sandbox.systemprompt.io:5432/demo?sslmode=disable";
    mount_ready_tenant(&server, "t-ext", db_url).await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-ext/external-db-access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tenant_id": "t-ext",
                "external_db_access": true,
                "database_url": db_url
            }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let progress = RecordingProgress::new();
    let tenant = TenantProvisioningService::new(&client)
        .finalize_tenant(&TenantId::new("t-ext"), true, &progress)
        .await
        .expect("finalize");

    assert!(tenant.external_db_access);
    assert_eq!(
        tenant.database_url.as_deref(),
        Some("postgres://u:p@db-sandbox.systemprompt.io:5432/demo?sslmode=require")
    );

    let labels = progress.labels();
    assert!(labels.contains(&"ExternalAccessStarted".to_owned()));
    assert!(labels.contains(&"ExternalAccessEnabled".to_owned()));
    assert!(!labels.contains(&"ExternalAccessFailed".to_owned()));
}

#[tokio::test]
async fn finalize_tenant_continues_when_external_access_fails() {
    let server = MockServer::start().await;
    let db_url = "postgres://u:p@internal.systemprompt.io:5432/demo";
    mount_ready_tenant(&server, "t-extfail", db_url).await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/tenants/t-extfail/external-db-access"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let progress = RecordingProgress::new();
    let tenant = TenantProvisioningService::new(&client)
        .finalize_tenant(&TenantId::new("t-extfail"), true, &progress)
        .await
        .expect("finalize");

    assert!(!tenant.external_db_access);
    assert_eq!(tenant.database_url, None);
    assert!(progress.labels().contains(&"ExternalAccessFailed".to_owned()));
}

#[tokio::test]
async fn finalize_tenant_surfaces_provisioning_failure() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-fail/events"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-fail/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "status": "failed", "message": "volume allocation failed" }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let progress = RecordingProgress::new();
    let err = TenantProvisioningService::new(&client)
        .finalize_tenant(&TenantId::new("t-fail"), false, &progress)
        .await
        .expect_err("must fail");

    assert!(err.to_string().contains("volume allocation failed"));
    let labels = progress.labels();
    assert!(labels.contains(&"ProvisioningStarted".to_owned()));
    assert!(!labels.contains(&"Provisioned".to_owned()));
}

#[test]
fn swap_to_external_host_targets_production() {
    let url = "postgres://u:p@internal.systemprompt.io:5432/db?sslmode=disable";
    assert_eq!(
        swap_to_external_host(url),
        "postgres://u:p@db.systemprompt.io:5432/db?sslmode=require"
    );
}

#[test]
fn swap_to_external_host_targets_sandbox() {
    let url = "postgres://u:p@pg.sandbox.internal:5432/db";
    assert_eq!(
        swap_to_external_host(url),
        "postgres://u:p@db-sandbox.systemprompt.io:5432/db"
    );
}

#[test]
fn swap_to_external_host_passes_through_unparseable_input() {
    assert_eq!(swap_to_external_host("not a url"), "not a url");
}
