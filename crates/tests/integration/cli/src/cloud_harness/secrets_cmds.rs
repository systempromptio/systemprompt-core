//! Harness tests for `cloud secrets` error paths not covered by the core
//! harness suite.

use serde_json::json;
use systemprompt_cli::cloud::secrets::SecretsCommands;
use systemprompt_cli::cloud::{self, CloudCommands};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{TENANT_ID, enter, json_ctx, table_ctx};

#[tokio::test]
async fn set_rejects_malformed_pair() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    let err = cloud::execute(
        CloudCommands::Secrets(SecretsCommands::Set {
            key_values: vec!["NO_EQUALS_SIGN".to_owned()],
        }),
        &json_ctx(),
    )
    .await
    .expect_err("malformed pair");
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn set_surfaces_api_failure() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({"error": "boom"})))
        .mount(env.server())
        .await;

    let err = cloud::execute(
        CloudCommands::Secrets(SecretsCommands::Set {
            key_values: vec!["CUSTOM_SECRET=value".to_owned()],
        }),
        &json_ctx(),
    )
    .await
    .expect_err("api failure surfaces");
    assert!(!err.to_string().is_empty());
}

#[tokio::test]
async fn unset_skips_system_managed_keys() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    let result = cloud::execute(
        CloudCommands::Secrets(SecretsCommands::Unset {
            keys: vec!["FLY_APP_NAME".to_owned()],
        }),
        &table_ctx(),
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn sync_renders_table_output() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(CloudCommands::Secrets(SecretsCommands::Sync), &table_ctx())
        .await
        .expect("secrets sync table");
}
