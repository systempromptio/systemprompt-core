//! Harness tests for the table-rendering and guard branches of
//! `cloud tenant delete` and `cloud tenant list`.

use serde_json::json;
use systemprompt_cli::cloud::tenant::{TenantCommands, TenantDeleteArgs};
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{OTHER_TENANT_ID, TENANT_ID, enter, interactive_ctx, json_ctx, table_ctx};

fn tenant_cmd(command: TenantCommands) -> CloudCommands {
    CloudCommands::Tenant {
        command: Some(command),
    }
}

#[tokio::test]
async fn delete_without_id_errors_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Delete(TenantDeleteArgs {
            id: None,
            yes: true,
        })),
        &json_ctx(),
    )
    .await
    .expect_err("delete needs --id non-interactive");
    assert!(err.to_string().contains("--id is required"));
}

#[tokio::test]
async fn delete_cloud_tenant_table_output() {
    let env = enter().await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        tenant_cmd(TenantCommands::Delete(TenantDeleteArgs {
            id: Some(TENANT_ID.to_owned()),
            yes: true,
        })),
        &table_ctx(),
    )
    .await
    .expect("delete cloud tenant table");
}

#[tokio::test]
async fn delete_cancelled_by_confirmation() {
    let env = enter().await;
    let ctx = interactive_ctx(["n"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Delete(TenantDeleteArgs {
            id: Some(OTHER_TENANT_ID.to_owned()),
            yes: false,
        })),
        &ctx,
    )
    .await
    .expect("delete cancelled");
    let store = TenantStore::load_from_path(&env.root().join(".systemprompt/tenants.json"))
        .expect("reload tenants");
    assert!(store.tenants.iter().any(|t| t.id == OTHER_TENANT_ID));
}

#[tokio::test]
async fn list_table_output_non_interactive() {
    let env = enter().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": { "id": "user_harness", "email": "harness@example.com", "name": "Harness" },
            "tenants": [{
                "id": TENANT_ID,
                "name": "Harness Prod",
                "hostname": "harness.example.com",
                "region": "iad",
                "external_db_access": true,
                "database_url": "postgres://int/db"
            }]
        })))
        .mount(env.server())
        .await;

    cloud::execute(tenant_cmd(TenantCommands::List), &table_ctx())
        .await
        .expect("tenant list table");
}

#[tokio::test]
async fn list_reports_no_tenants_when_empty() {
    let env = enter().await;
    TenantStore::default()
        .save_to_path(&get_cloud_paths().resolve(CloudPath::Tenants))
        .expect("clear tenants");
    env.server().reset().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    cloud::execute(tenant_cmd(TenantCommands::List), &table_ctx())
        .await
        .expect("tenant list empty");
}
