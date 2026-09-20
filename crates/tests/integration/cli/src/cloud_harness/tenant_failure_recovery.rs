//! Tenant mutations preserve the local cache until the cloud accepts them.

use serde_json::json;
use systemprompt_cli::cloud::tenant::{TenantCommands, TenantDeleteArgs, TenantRotateArgs};
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::{CloudPath, TenantStore, get_cloud_paths};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{OTHER_TENANT_ID, TENANT_ID, enter, json_ctx, mount_token_exchange, seed_tenants};

fn tenant(command: TenantCommands) -> CloudCommands {
    CloudCommands::Tenant {
        command: Some(command),
    }
}

async fn execute(command: CloudCommands) -> Result<(), String> {
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        cloud::execute(command, &json_ctx()),
    )
    .await
    .expect("owned tenant command completes within ten seconds")
    .map_err(|error| format!("{error:#}"))
}

#[tokio::test]
async fn rotation_failure_preserves_both_accounts_then_retry_updates_only_the_selected_tenant() {
    let env = enter().await;
    seed_tenants(env.root());
    let tenants_path = get_cloud_paths().resolve(CloudPath::Tenants);
    let original = std::fs::read(&tenants_path).expect("original tenant cache");
    let original_store =
        TenantStore::load_from_path(&tenants_path).expect("original tenant records");
    let original_other = serde_json::to_value(
        original_store
            .tenants
            .iter()
            .find(|tenant| tenant.id == OTHER_TENANT_ID)
            .expect("original unrelated account"),
    )
    .expect("serialize original unrelated account");
    Mock::given(method("POST"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/rotate-credentials"
        )))
        .and(header("authorization", "Bearer tenant_bearer"))
        .respond_with(ResponseTemplate::new(503).set_body_string("owned rotation unavailable"))
        .expect(1)
        .mount(env.server())
        .await;

    let failed = execute(tenant(TenantCommands::RotateCredentials(
        TenantRotateArgs {
            id: Some(TENANT_ID.to_owned()),
            yes: true,
        },
    )))
    .await
    .expect_err("rejected rotation is visible");
    assert!(failed.contains("owned rotation unavailable"));
    assert_eq!(
        std::fs::read(&tenants_path).expect("tenant cache after failed rotation"),
        original,
        "a rejected remote rotation must not rewrite cached credentials"
    );

    env.server().reset().await;
    mount_token_exchange(env.server()).await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/api/v1/tenants/{TENANT_ID}/rotate-credentials"
        )))
        .and(header("authorization", "Bearer tenant_bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "rotated",
            "message": "owned retry",
            "internal_database_url": "postgres://repaired-internal/db",
            "external_database_url": "postgres://repaired-external/db"
        })))
        .expect(1)
        .mount(env.server())
        .await;
    execute(tenant(TenantCommands::RotateCredentials(
        TenantRotateArgs {
            id: Some(TENANT_ID.to_owned()),
            yes: true,
        },
    )))
    .await
    .expect("repaired rotation succeeds");

    let repaired = TenantStore::load_from_path(&tenants_path).expect("repaired tenant cache");
    let selected = repaired
        .tenants
        .iter()
        .find(|tenant| tenant.id == TENANT_ID)
        .expect("selected cloud tenant retained");
    assert_eq!(
        selected.internal_database_url.as_deref(),
        Some("postgres://repaired-internal/db")
    );
    assert_eq!(
        selected.database_url.as_deref(),
        Some("postgres://repaired-external/db")
    );
    let other = repaired
        .tenants
        .iter()
        .find(|tenant| tenant.id == OTHER_TENANT_ID)
        .expect("other account retained");
    assert_eq!(
        serde_json::to_value(other).expect("serialize unrelated account after retry"),
        original_other,
        "the selected-account mutation must preserve the full unrelated account record"
    );
}

#[tokio::test]
async fn deletion_failure_preserves_cache_then_retry_removes_only_the_selected_tenant() {
    let env = enter().await;
    seed_tenants(env.root());
    let tenants_path = get_cloud_paths().resolve(CloudPath::Tenants);
    let original = std::fs::read(&tenants_path).expect("original tenant cache");
    let original_store =
        TenantStore::load_from_path(&tenants_path).expect("original tenant records");
    let original_other = serde_json::to_value(
        original_store
            .tenants
            .iter()
            .find(|tenant| tenant.id == OTHER_TENANT_ID)
            .expect("original unrelated account"),
    )
    .expect("serialize original unrelated account");
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}")))
        .and(header("authorization", "Bearer tenant_bearer"))
        .respond_with(ResponseTemplate::new(409).set_body_string("owned deletion blocked"))
        .expect(1)
        .mount(env.server())
        .await;

    let failed = execute(tenant(TenantCommands::Delete(TenantDeleteArgs {
        id: Some(TENANT_ID.to_owned()),
        yes: true,
    })))
    .await
    .expect_err("rejected deletion is visible");
    assert!(failed.contains("owned deletion blocked"));
    assert_eq!(
        std::fs::read(&tenants_path).expect("tenant cache after failed deletion"),
        original,
        "a rejected cloud deletion must not drop the local account"
    );

    env.server().reset().await;
    mount_token_exchange(env.server()).await;
    Mock::given(method("DELETE"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}")))
        .and(header("authorization", "Bearer tenant_bearer"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(env.server())
        .await;
    execute(tenant(TenantCommands::Delete(TenantDeleteArgs {
        id: Some(TENANT_ID.to_owned()),
        yes: true,
    })))
    .await
    .expect("repaired deletion succeeds");

    let repaired = TenantStore::load_from_path(&tenants_path).expect("tenant cache after retry");
    assert!(repaired.tenants.iter().all(|tenant| tenant.id != TENANT_ID));
    let other = repaired
        .tenants
        .iter()
        .find(|tenant| tenant.id == OTHER_TENANT_ID)
        .expect("unrelated account retained");
    assert_eq!(
        serde_json::to_value(other).expect("serialize unrelated account after retry"),
        original_other,
        "the selected-account mutation must preserve the full unrelated account record"
    );
}
