//! Harness tests for `cloud auth login`: the non-interactive guard, and
//! `complete_login` against accounts whose `/auth/me` payload exercises the
//! customer, plan, and empty-tenant branches of the login renderer.

use serde_json::json;
use systemprompt_cli::cloud::auth::{AuthCommands, complete_login};
use systemprompt_cli::cloud::{self, CloudCommands, Environment};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{FAR_FUTURE_JWT, USER_EMAIL, enter, json_ctx};

async fn mount_auth_me(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn stored_tenants() -> TenantStore {
    TenantStore::load_from_path(&get_cloud_paths().resolve(CloudPath::Tenants))
        .expect("tenant store written by login")
}

#[tokio::test]
async fn login_requires_interactive_mode() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Auth(AuthCommands::Login {
            environment: Environment::Production,
        }),
        &json_ctx(),
    )
    .await
    .expect_err("browser OAuth cannot run without a terminal");
    assert!(
        err.to_string().contains("interactive"),
        "expected an interactive-mode error, got: {err}"
    );
}

#[tokio::test]
async fn complete_login_persists_customer_account_tenant() {
    let env = enter().await;
    env.server().reset().await;
    mount_auth_me(
        env.server(),
        json!({
            "user": { "id": "user_rich", "email": USER_EMAIL, "name": "Rich Harness" },
            "customer": { "id": "cus_harness" },
            "tenants": [{
                "id": "t-rich",
                "name": "Rich Prod",
                "hostname": "rich.example.com",
                "region": "lhr",
                "external_db_access": false,
                "database_url": "postgres://int/rich",
                "plan": { "name": "launch", "memory_mb": 1024, "volume_gb": 5 }
            }]
        }),
    )
    .await;

    complete_login(
        &env.server().uri(),
        FAR_FUTURE_JWT.to_owned(),
        &json_ctx().cli,
    )
    .await
    .expect("login with a fully populated cloud account");

    let store = stored_tenants();
    assert_eq!(store.tenants.len(), 1);
    let tenant = &store.tenants[0];
    assert_eq!(tenant.id.as_str(), "t-rich");
    assert_eq!(tenant.name, "Rich Prod");
    assert_eq!(tenant.hostname.as_deref(), Some("rich.example.com"));
    assert_eq!(tenant.region.as_deref(), Some("lhr"));
}

#[tokio::test]
async fn complete_login_without_tenants_writes_empty_store() {
    let env = enter().await;
    env.server().reset().await;
    mount_auth_me(
        env.server(),
        json!({
            "user": { "id": "user_bare", "email": USER_EMAIL, "name": null },
            "tenants": []
        }),
    )
    .await;

    complete_login(
        &env.server().uri(),
        FAR_FUTURE_JWT.to_owned(),
        &json_ctx().cli,
    )
    .await
    .expect("login for an account with no tenants");

    assert!(stored_tenants().tenants.is_empty());
}
