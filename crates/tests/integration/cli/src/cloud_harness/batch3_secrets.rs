//! Harness tests for the human-readable (`table`) rendering branches of the
//! `cloud secrets` handlers: set/unset/cleanup success loops, API-error
//! surfacing, and the empty-input guards.

use systemprompt_cli::cloud::secrets::SecretsCommands;
use systemprompt_cli::cloud::{self, CloudCommands};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{TENANT_ID, enter, table_ctx};

fn secrets_cmd(command: SecretsCommands) -> CloudCommands {
    CloudCommands::Secrets(command)
}

#[tokio::test]
async fn set_table_output_prints_each_key() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        secrets_cmd(SecretsCommands::Set {
            key_values: vec!["CUSTOM_SECRET=value".to_owned()],
        }),
        &table_ctx(),
    )
    .await
    .expect("set secrets table");
}

#[tokio::test]
async fn set_table_output_surfaces_api_failure() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    let err = cloud::execute(
        secrets_cmd(SecretsCommands::Set {
            key_values: vec!["CUSTOM_SECRET=value".to_owned()],
        }),
        &table_ctx(),
    )
    .await
    .expect_err("set failure bubbles in table mode");
    assert!(err.to_string().contains("Failed to set secrets"));
}

#[tokio::test]
async fn set_table_output_warns_on_rejected_keys() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets")))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        secrets_cmd(SecretsCommands::Set {
            key_values: vec![
                "CUSTOM_SECRET=value".to_owned(),
                "FLY_APP_NAME=managed".to_owned(),
            ],
        }),
        &table_ctx(),
    )
    .await
    .expect("set with rejected key");
}

#[tokio::test]
async fn sync_table_output_warns_on_empty_secrets() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    let secrets_path = env.root().join(".systemprompt/profiles/local/secrets.json");
    let original = std::fs::read_to_string(&secrets_path).expect("read secrets");
    std::fs::write(&secrets_path, "{}").expect("empty secrets");

    cloud::execute(secrets_cmd(SecretsCommands::Sync), &table_ctx())
        .await
        .expect("sync empty secrets");

    std::fs::write(&secrets_path, original).expect("restore secrets");
}

#[tokio::test]
async fn unset_empty_keys_errors() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    let err = cloud::execute(
        secrets_cmd(SecretsCommands::Unset { keys: Vec::new() }),
        &table_ctx(),
    )
    .await
    .expect_err("no keys provided");
    assert!(err.to_string().contains("No keys"));
}

#[tokio::test]
async fn unset_table_output_removes_key() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/secrets/CUSTOM_SECRET"
        )))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(
        secrets_cmd(SecretsCommands::Unset {
            keys: vec!["custom_secret".to_owned()],
        }),
        &table_ctx(),
    )
    .await
    .expect("unset removes key in table mode");
}

#[tokio::test]
async fn unset_table_output_bails_when_all_fail() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/secrets/GONE")))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    let err = cloud::execute(
        secrets_cmd(SecretsCommands::Unset {
            keys: vec!["GONE".to_owned()],
        }),
        &table_ctx(),
    )
    .await
    .expect_err("all removals failed");
    assert!(err.to_string().contains("Failed to remove any secrets"));
}

#[tokio::test]
async fn cleanup_table_output_removes_system_managed() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/secrets/SYSTEMPROMPT_API_URL"
        )))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    cloud::execute(secrets_cmd(SecretsCommands::Cleanup), &table_ctx())
        .await
        .expect("cleanup table output");
}

#[tokio::test]
async fn cleanup_table_output_warns_on_failure() {
    let env = enter().await;
    if !env.profile_ready() {
        return;
    }
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/secrets/SYSTEMPROMPT_API_URL"
        )))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(env.server())
        .await;

    cloud::execute(secrets_cmd(SecretsCommands::Cleanup), &table_ctx())
        .await
        .expect("cleanup tolerates removal failure");
}
