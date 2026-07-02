//! Harness tests for `cloud domain` set/status/remove against the mock
//! control plane.

use serde_json::json;
use systemprompt_cli::cloud::domain::DomainCommands;
use systemprompt_cli::cloud::{self, CloudCommands};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{TENANT_ID, enter, interactive_ctx, json_ctx};

fn domain_body(verified: bool, with_timestamps: bool) -> serde_json::Value {
    let mut body = json!({
        "domain": "example.com",
        "status": if verified { "active" } else { "pending" },
        "verified": verified,
        "dns_target": "harness.example.com",
        "dns_instructions": {
            "record_type": "CNAME",
            "host": "example.com",
            "value": "harness.example.com",
            "ttl": 300
        }
    });
    if with_timestamps {
        body["created_at"] = json!("2026-07-01T00:00:00Z");
        body["verified_at"] = json!("2026-07-02T00:00:00Z");
    }
    body
}

async fn mount_get_domain(server: &MockServer, verified: bool, with_timestamps: bool) {
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "data": domain_body(verified, with_timestamps) })),
        )
        .mount(server)
        .await;
}

async fn mount_get_domain_404(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "error": "not_found" })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn set_domain_success() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": domain_body(false, false) })),
        )
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Domain(DomainCommands::Set {
            domain: "example.com".to_owned(),
        }),
        &json_ctx(),
    )
    .await
    .expect("domain set");
}

#[tokio::test]
async fn set_domain_failure_bails() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    let err = cloud::execute(
        CloudCommands::Domain(DomainCommands::Set {
            domain: "example.com".to_owned(),
        }),
        &json_ctx(),
    )
    .await
    .expect_err("set fails");
    assert!(err.to_string().contains("custom domain"));
}

#[tokio::test]
async fn status_verified_and_unverified() {
    let env = enter().await;
    mount_get_domain(env.server(), true, true).await;
    cloud::execute(CloudCommands::Domain(DomainCommands::Status), &json_ctx())
        .await
        .expect("verified status");

    env.server().reset().await;
    super::seed_tenants(env.root());
    mount_get_domain(env.server(), false, false).await;
    mount_token(env.server()).await;
    cloud::execute(CloudCommands::Domain(DomainCommands::Status), &json_ctx())
        .await
        .expect("unverified status");
}

async fn mount_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/v1/core/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tenant_bearer",
            "expires_in": 600
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn status_handles_no_domain_configured() {
    let env = enter().await;
    mount_get_domain_404(env.server()).await;
    cloud::execute(CloudCommands::Domain(DomainCommands::Status), &json_ctx())
        .await
        .expect("404 tolerated");
}

#[tokio::test]
async fn status_bails_on_server_error() {
    let env = enter().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    let err = cloud::execute(CloudCommands::Domain(DomainCommands::Status), &json_ctx())
        .await
        .expect_err("500 fails");
    assert!(err.to_string().contains("domain status"));
}

#[tokio::test]
async fn remove_with_yes_deletes() {
    let env = enter().await;
    mount_get_domain(env.server(), true, false).await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Domain(DomainCommands::Remove { yes: true }),
        &json_ctx(),
    )
    .await
    .expect("domain remove");
}

#[tokio::test]
async fn remove_no_domain_is_noop() {
    let env = enter().await;
    mount_get_domain_404(env.server()).await;
    cloud::execute(
        CloudCommands::Domain(DomainCommands::Remove { yes: true }),
        &json_ctx(),
    )
    .await
    .expect("remove without domain");
}

#[tokio::test]
async fn remove_without_yes_errors_non_interactive() {
    let env = enter().await;
    mount_get_domain(env.server(), true, false).await;
    let err = cloud::execute(
        CloudCommands::Domain(DomainCommands::Remove { yes: false }),
        &json_ctx(),
    )
    .await
    .expect_err("needs --yes");
    assert!(err.to_string().contains("--yes"));
}

#[tokio::test]
async fn remove_interactive_cancel_and_confirm() {
    let env = enter().await;
    mount_get_domain(env.server(), true, false).await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        CloudCommands::Domain(DomainCommands::Remove { yes: false }),
        &interactive_ctx(["n"]),
    )
    .await
    .expect("cancelled remove");

    cloud::execute(
        CloudCommands::Domain(DomainCommands::Remove { yes: false }),
        &interactive_ctx(["y"]),
    )
    .await
    .expect("confirmed remove");
}

#[tokio::test]
async fn remove_delete_failure_bails() {
    let env = enter().await;
    mount_get_domain(env.server(), true, false).await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/custom-domain")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    let err = cloud::execute(
        CloudCommands::Domain(DomainCommands::Remove { yes: true }),
        &json_ctx(),
    )
    .await
    .expect_err("delete fails");
    assert!(err.to_string().contains("remove custom domain"));
}
