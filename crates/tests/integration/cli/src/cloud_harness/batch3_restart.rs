//! Harness tests for the table-rendering and edge branches of `cloud restart`.

use serde_json::json;
use systemprompt_cli::cloud::{self, CloudCommands};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{TENANT_ID, enter, interactive_ctx, json_ctx, table_ctx};

fn restart_cmd(tenant: Option<&str>, yes: bool) -> CloudCommands {
    CloudCommands::Restart {
        tenant: tenant.map(str::to_owned),
        yes,
    }
}

#[tokio::test]
async fn restart_table_output_success() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/restart")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "restarting" })))
        .mount(env.server())
        .await;

    cloud::execute(restart_cmd(Some(TENANT_ID), true), &table_ctx())
        .await
        .expect("restart table");
}

#[tokio::test]
async fn restart_table_output_surfaces_api_failure() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/restart")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    let err = cloud::execute(restart_cmd(Some(TENANT_ID), true), &table_ctx())
        .await
        .expect_err("restart failure surfaces");
    assert!(err.to_string().contains("Failed to restart tenant"));
}

#[tokio::test]
async fn restart_cancelled_by_confirmation() {
    let _env = enter().await;
    let ctx = interactive_ctx(["n"]);
    cloud::execute(restart_cmd(Some(TENANT_ID), false), &ctx)
        .await
        .expect("restart cancelled");
}

#[tokio::test]
async fn restart_tenant_absent_from_store_uses_id_as_name() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/tenants/t-absent/restart"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "status": "restarting" })))
        .mount(env.server())
        .await;

    cloud::execute(restart_cmd(Some("t-absent"), true), &json_ctx())
        .await
        .expect("restart tenant absent from store");
}
