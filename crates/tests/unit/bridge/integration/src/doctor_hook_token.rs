//! The provisioned arms of the doctor `hook token mint` check: with OAuth
//! client creds stored (metadata on disk, secret in the headless keyutils
//! store), the check must pass on a 200 token grant and fail with the
//! operator-action diagnosis on a rejection.

use systemprompt_bridge::auth::plugin_oauth::{self, OAuthClientCreds};
use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::auth::check_hook_token_mint;
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_identifiers::{ClientId, ValidatedUrl};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn use_keyutils_store() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let store = linux_keyutils_keyring_store::Store::new().unwrap();
        keyring_core::set_default_store(store);
    });
}

fn unique_client(prefix: &str) -> String {
    format!("{prefix}-{}", std::process::id())
}

fn check_with_endpoint(client_id: &str, token_status: u16) -> systemprompt_bridge::cli::doctor::Check {
    use_keyutils_store();
    let temp = TempDir::new().unwrap();
    temp_env::with_var("XDG_CACHE_HOME", Some(temp.path().as_os_str()), || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let server = MockServer::start().await;
            let template = if token_status == 200 {
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_raw(
                        r#"{"access_token":"jwt.value","token_type":"Bearer","expires_in":3600,"scope":"hook:govern hook:track"}"#,
                        "application/json",
                    )
            } else {
                ResponseTemplate::new(token_status).set_body_raw(
                    r#"{"error":"invalid_scope"}"#,
                    "application/json",
                )
            };
            Mock::given(method("POST"))
                .and(path("/v1/oauth/token"))
                .respond_with(template)
                .mount(&server)
                .await;

            plugin_oauth::store_creds(&OAuthClientCreds {
                client_id: ClientId::new(client_id),
                client_secret: "shh-secret".into(),
                token_endpoint: format!("{}/v1/oauth/token", server.uri()),
                scopes: vec!["hook:govern".into(), "hook:track".into()],
            })
            .expect("store creds");

            let gateway = GatewayClient::new(ValidatedUrl::new(server.uri()));
            check_hook_token_mint(&gateway).await
        })
    })
}

#[test]
fn a_provisioned_client_and_a_200_grant_pass_the_check() {
    let id = unique_client("doctor-hook-ok");
    let check = check_with_endpoint(&id, 200);
    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(check.detail.contains(&id), "{}", check.detail);
    assert!(
        check.detail.contains("hook:govern hook:track"),
        "{}",
        check.detail
    );
}

#[test]
fn a_rejected_grant_fails_with_the_operator_action_diagnosis() {
    let check = check_with_endpoint(&unique_client("doctor-hook-rej"), 403);
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(check.detail.contains("status=403"), "{}", check.detail);
    assert!(check.detail.contains("invalid_scope"), "{}", check.detail);
    assert!(
        check.detail.contains("operator action"),
        "{}",
        check.detail
    );
}
