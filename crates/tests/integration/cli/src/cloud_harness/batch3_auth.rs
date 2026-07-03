//! Harness tests for the human-readable rendering and edge branches of
//! `cloud auth whoami` / `cloud auth logout`.

use chrono::Utc;
use serde_json::json;
use systemprompt_cli::cloud::auth::{AuthCommands, LogoutArgs};
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::{CloudPath, get_cloud_paths};
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{USER_EMAIL, enter, interactive_ctx, json_ctx, mount_list_tenants, table_ctx};

fn write_creds(api_url: &str, token: &str) {
    let creds = json!({
        "api_token": token,
        "api_url": api_url,
        "authenticated_at": Utc::now().to_rfc3339(),
        "user_email": USER_EMAIL,
        "last_validated_at": null,
    });
    std::fs::write(
        get_cloud_paths().resolve(CloudPath::Credentials),
        serde_json::to_string_pretty(&creds).unwrap(),
    )
    .expect("write credentials");
}

#[tokio::test]
async fn whoami_table_output_renders_identity() {
    let env = enter().await;
    mount_list_tenants(env.server()).await;
    cloud::execute(CloudCommands::Auth(AuthCommands::Whoami), &table_ctx())
        .await
        .expect("whoami table");
}

#[tokio::test]
async fn whoami_reports_not_logged_in() {
    let _env = enter().await;
    std::fs::remove_file(get_cloud_paths().resolve(CloudPath::Credentials))
        .expect("remove credentials");
    cloud::execute(CloudCommands::Auth(AuthCommands::Whoami), &table_ctx())
        .await
        .expect("whoami without credentials");
}

#[tokio::test]
async fn whoami_reports_expired_token() {
    let env = enter().await;
    write_creds(&env.server().uri(), "e30.eyJleHAiOjF9.sig");
    cloud::execute(CloudCommands::Auth(AuthCommands::Whoami), &table_ctx())
        .await
        .expect("whoami expired token");
}

#[tokio::test]
async fn logout_table_output_removes_credentials() {
    let env = enter().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/activity"))
        .respond_with(ResponseTemplate::new(204))
        .mount(env.server())
        .await;

    let creds_path = get_cloud_paths().resolve(CloudPath::Credentials);
    cloud::execute(
        CloudCommands::Auth(AuthCommands::Logout(LogoutArgs { yes: true })),
        &table_ctx(),
    )
    .await
    .expect("logout table");
    assert!(!creds_path.exists());
}

#[tokio::test]
async fn logout_reports_already_logged_out() {
    let _env = enter().await;
    std::fs::remove_file(get_cloud_paths().resolve(CloudPath::Credentials))
        .expect("remove credentials");
    cloud::execute(
        CloudCommands::Auth(AuthCommands::Logout(LogoutArgs { yes: true })),
        &table_ctx(),
    )
    .await
    .expect("logout already logged out");
}

#[tokio::test]
async fn logout_without_yes_errors_non_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Auth(AuthCommands::Logout(LogoutArgs { yes: false })),
        &json_ctx(),
    )
    .await
    .expect_err("logout needs --yes");
    assert!(err.to_string().contains("--yes"));
}

#[tokio::test]
async fn logout_cancelled_keeps_credentials() {
    let _env = enter().await;
    let ctx = interactive_ctx(["n"]);
    cloud::execute(
        CloudCommands::Auth(AuthCommands::Logout(LogoutArgs { yes: false })),
        &ctx,
    )
    .await
    .expect("logout cancelled");
    assert!(get_cloud_paths().resolve(CloudPath::Credentials).exists());
}
