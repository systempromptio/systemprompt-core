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

    assert_eq!(tenant.id.as_str(), "t-prov");
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
async fn finalize_tenant_errors_when_secrets_url_missing() {
    let server = MockServer::start().await;
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-nosecret/events"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-nosecret/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "status": "ready" }
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let progress = RecordingProgress::new();
    let err = TenantProvisioningService::new(&client)
        .finalize_tenant(&TenantId::new("t-nosecret"), false, &progress)
        .await
        .expect_err("missing secrets url must fail");
    assert!(err.to_string().contains("secrets URL is missing"));
}

#[tokio::test]
async fn finalize_tenant_errors_when_tenant_absent_from_user() {
    let server = MockServer::start().await;
    let db_url = "postgres://u:p@internal.systemprompt.io:5432/demo";
    token_mock(&server).await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-ghost/events"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-ghost/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "status": "ready",
                "secrets_url": format!("{}/api/v1/tenants/t-ghost/secrets-bundle", server.uri())
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-ghost/secrets-bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "database_url": db_url,
            "internal_database_url": db_url,
            "app_url": "https://demo.fly.dev"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": { "id": "user-1", "email": "dev@example.com" },
            "tenants": []
        })))
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let progress = RecordingProgress::new();
    let err = TenantProvisioningService::new(&client)
        .finalize_tenant(&TenantId::new("t-ghost"), false, &progress)
        .await
        .expect_err("absent tenant must fail");
    assert!(err.to_string().contains("New tenant not found"));
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
    assert!(
        progress
            .labels()
            .contains(&"ExternalAccessFailed".to_owned())
    );
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

#[tokio::test]
async fn finalize_tenant_relays_sse_progress_messages() {
    let server = MockServer::start().await;
    let db_url = "postgres://u:p@internal.systemprompt.io:5432/demo";
    mount_ready_tenant(&server, "t-sse", db_url).await;
    let sse_body = concat!(
        "event: provisioning\n",
        "data: {\"tenant_id\":\"t-sse\",\"event_type\":\"vm_provisioning_started\",",
        "\"status\":\"working\",\"message\":\"allocating vm\"}\n\n",
        "event: provisioning\n",
        "data: {\"tenant_id\":\"t-sse\",\"event_type\":\"tenant_ready\",\"status\":\"ready\"}\n\n"
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants/t-sse/events"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse_body.as_bytes().to_vec(), "text/event-stream"),
        )
        .with_priority(1)
        .mount(&server)
        .await;

    let client = CloudApiClient::new(&server.uri(), "operator").unwrap();
    let progress = RecordingProgress::new();
    let tenant = TenantProvisioningService::new(&client)
        .finalize_tenant(&TenantId::new("t-sse"), false, &progress)
        .await
        .expect("finalize");

    assert_eq!(tenant.id.as_str(), "t-sse");
    let labels = progress.labels();
    assert!(
        labels.contains(&"ProvisioningUpdate".to_owned()),
        "SSE message must surface as a progress update, got {labels:?}"
    );
    assert!(labels.contains(&"Provisioned".to_owned()));
}

#[tokio::test]
async fn provision_runs_checkout_then_finalizes_tenant() {
    unsafe { std::env::set_var("BROWSER", "/bin/true") };
    let server = MockServer::start().await;
    let db_url = "postgres://u:p@internal.systemprompt.io:5432/demo";
    mount_ready_tenant(&server, "t-full", db_url).await;
    Mock::given(method("POST"))
        .and(path("/api/v1/checkout"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "checkout_url": "http://127.0.0.1:9/never-opened",
            "transaction_id": "tx-full",
            "checkout_session_id": "cs-full"
        })))
        .mount(&server)
        .await;

    let api_url = server.uri();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(90);
    let (progress_labels, provisioned) = loop {
        let api_url_cl = api_url.clone();
        let flow = tokio::spawn(async move {
            let client = CloudApiClient::new(&api_url_cl, "operator").expect("client");
            let progress = RecordingProgress::new();
            let plan = systemprompt_cloud::tenants::TenantCreatePlan {
                price_id: systemprompt_identifiers::PriceId::new("price-basic"),
                region: "lhr".to_owned(),
                redirect_uri: "http://127.0.0.1:8766/callback".to_owned(),
                external_db_access: false,
                templates: systemprompt_cloud::CheckoutTemplates {
                    success_html: "<p>ok {{TENANT_ID}}</p>",
                    error_html: "<p>err</p>",
                    waiting_html: "<p>wait</p>",
                },
            };
            let result = TenantProvisioningService::new(&client)
                .provision(&plan, &progress)
                .await;
            (progress.labels(), result)
        });

        loop {
            if flow.is_finished() {
                break;
            }
            let callback = reqwest::get(
                "http://127.0.0.1:8766/callback?transaction_id=tx-full&tenant_id=t-full&status=completed",
            )
            .await;
            if callback.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        let (labels, result) = flow.await.expect("join");
        match result {
            Ok(provisioned) => break (labels, provisioned),
            Err(e)
                if matches!(&e, systemprompt_cloud::error::CloudError::Io(io)
                    if io.kind() == std::io::ErrorKind::AddrInUse) =>
            {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "callback port stayed in use"
                );
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            },
            Err(other) => panic!("provision failed: {other:?}"),
        }
    };

    assert_eq!(provisioned.tenant.id.as_str(), "t-full");
    assert!(!provisioned.needs_deploy);
    assert!(progress_labels.contains(&"CheckoutSessionStarted".to_owned()));
    assert!(progress_labels.contains(&"CheckoutSessionCreated".to_owned()));
    assert!(progress_labels.contains(&"CheckoutComplete".to_owned()));
    assert!(progress_labels.contains(&"TenantSynced".to_owned()));
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
