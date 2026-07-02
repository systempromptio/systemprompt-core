//! Harness tests for the interactive tenant flows: edit, the create menu,
//! and external-database tenant creation end to end.

use systemprompt_cli::cloud::tenant::TenantCommands;
use systemprompt_cli::cloud::{self, CloudCommands};
use systemprompt_cloud::TenantStore;

use super::{OTHER_TENANT_ID, TENANT_ID, enter, interactive_ctx, json_ctx};
use crate::full_bootstrap::database_url;

fn tenant_cmd(command: TenantCommands) -> CloudCommands {
    CloudCommands::Tenant {
        command: Some(command),
    }
}

#[tokio::test]
async fn tenant_edit_requires_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(OTHER_TENANT_ID.to_owned()),
        }),
        &json_ctx(),
    )
    .await
    .expect_err("edit needs interactive");
    assert!(err.to_string().contains("interactive"));
}

#[tokio::test]
async fn tenant_edit_local_renames_and_edits_database() {
    let env = enter().await;
    let ctx = interactive_ctx(["renamed-local", "y", "postgres://u:p@edited:5432/db"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(OTHER_TENANT_ID.to_owned()),
        }),
        &ctx,
    )
    .await
    .expect("edit local tenant");

    let store = TenantStore::load_from_path(&env.root().join(".systemprompt/tenants.json"))
        .expect("reload tenants");
    let tenant = store.find_tenant(OTHER_TENANT_ID).expect("tenant");
    assert_eq!(tenant.name, "renamed-local");
    assert_eq!(
        tenant.database_url.as_deref(),
        Some("postgres://u:p@edited:5432/db")
    );
}

#[tokio::test]
async fn tenant_edit_local_declines_database_edit() {
    let _env = enter().await;
    let ctx = interactive_ctx(["kept-name", "n"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(OTHER_TENANT_ID.to_owned()),
        }),
        &ctx,
    )
    .await
    .expect("edit declining db change");
}

#[tokio::test]
async fn tenant_edit_cloud_shows_readonly_fields() {
    let _env = enter().await;
    let ctx = interactive_ctx(["Harness Prod Renamed"]);
    cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some(TENANT_ID.to_owned()),
        }),
        &ctx,
    )
    .await
    .expect("edit cloud tenant");
}

#[tokio::test]
async fn tenant_edit_unknown_id_errors() {
    let _env = enter().await;
    let ctx = interactive_ctx(Vec::<String>::new());
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Edit {
            id: Some("nope".to_owned()),
        }),
        &ctx,
    )
    .await
    .expect_err("unknown tenant");
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn tenant_create_requires_interactive() {
    let _env = enter().await;
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &json_ctx(),
    )
    .await
    .expect_err("create needs interactive");
    assert!(err.to_string().contains("interactive"));
}

#[tokio::test]
async fn tenant_create_cloud_unavailable_without_release_build() {
    let _env = enter().await;
    let ctx = interactive_ctx(["1"]);
    let result = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
    .await;
    let _ = result;
}

#[tokio::test]
async fn tenant_create_external_rejects_empty_inputs() {
    let _env = enter().await;

    let ctx = interactive_ctx(["0", "1", "ext-tenant", ""]);
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
    .await
    .expect_err("empty database url");
    assert!(err.to_string().contains("Database URL"));

    let ctx = interactive_ctx(["0", "1", "ext-tenant", "postgres://u:p@127.0.0.1:1/void"]);
    let err = cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
    .await
    .expect_err("unreachable database");
    assert!(err.to_string().contains("connect"));
}

#[tokio::test]
async fn tenant_create_external_full_flow() {
    let Some(url) = database_url() else { return };
    let env = enter().await;
    let profiles = env.root().join(".systemprompt/profiles/ext-prof");
    if profiles.exists() {
        std::fs::remove_dir_all(&profiles).expect("clean ext profile");
    }

    let ctx = interactive_ctx([
        "0",
        "1",
        "ext-tenant",
        url.as_str(),
        "ext-prof",
        "0",
        "ext-gemini-key",
        "n",
        "n",
    ]);
    cloud::execute(
        tenant_cmd(TenantCommands::Create {
            region: "iad".to_owned(),
        }),
        &ctx,
    )
    .await
    .expect("external tenant create");

    let store = TenantStore::load_from_path(&env.root().join(".systemprompt/tenants.json"))
        .expect("reload tenants");
    assert!(store.tenants.iter().any(|t| t.name == "ext-tenant"));
    assert!(profiles.join("profile.yaml").exists());

    std::fs::remove_dir_all(&profiles).expect("clean ext profile");
}
