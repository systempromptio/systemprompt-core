//! Harness tests for `cloud sync` admin-user routing and the interactive
//! sync menu.

use systemprompt_cli::cloud::sync::{AdminUserSyncArgs, SyncCommands};
use systemprompt_cli::cloud::{self, CloudCommands};

use super::{enter, interactive_ctx, json_ctx};

fn sync_cmd(command: SyncCommands) -> CloudCommands {
    CloudCommands::Sync {
        command: Some(command),
    }
}

#[tokio::test]
async fn admin_user_database_url_requires_profile() {
    let _env = enter().await;
    let err = cloud::execute(
        sync_cmd(SyncCommands::AdminUser(AdminUserSyncArgs {
            verbose: false,
            profile: None,
            database_url: Some("postgres://u:p@localhost/db".to_owned()),
        })),
        &json_ctx(),
    )
    .await
    .expect_err("--database-url requires --profile");
    assert!(err.to_string().contains("--profile"));
}

#[tokio::test]
async fn admin_user_unknown_profile_errors() {
    let _env = enter().await;
    let err = cloud::execute(
        sync_cmd(SyncCommands::AdminUser(AdminUserSyncArgs {
            verbose: true,
            profile: Some("ghost-profile".to_owned()),
            database_url: None,
        })),
        &json_ctx(),
    )
    .await
    .expect_err("unknown profile");
    assert!(err.to_string().contains("ghost-profile"));
}

#[tokio::test]
async fn admin_user_reports_connection_failure_for_profile_override() {
    let _env = enter().await;
    cloud::execute(
        sync_cmd(SyncCommands::AdminUser(AdminUserSyncArgs {
            verbose: false,
            profile: Some("local".to_owned()),
            database_url: Some("postgres://nobody:nothing@127.0.0.1:1/void".to_owned()),
        })),
        &json_ctx(),
    )
    .await
    .expect("connection failure is reported, not fatal");
}

#[tokio::test]
async fn admin_user_all_profiles_discovery_runs() {
    let _env = enter().await;
    cloud::execute(
        sync_cmd(SyncCommands::AdminUser(AdminUserSyncArgs {
            verbose: true,
            profile: None,
            database_url: None,
        })),
        &json_ctx(),
    )
    .await
    .expect("all-profile sync runs");
}

#[tokio::test]
async fn interactive_menu_drives_push_and_pull() {
    let env = enter().await;
    let _ = env;

    for direction in ["0", "1"] {
        let ctx = interactive_ctx([direction, "0"]);
        let result = cloud::execute(CloudCommands::Sync { command: None }, &ctx).await;
        let _ = result;
    }
}

#[tokio::test]
async fn push_dry_run_reaches_sync_service() {
    let env = enter().await;
    let _ = env;
    let result = cloud::execute(
        sync_cmd(SyncCommands::Push(
            systemprompt_cli::cloud::sync::SyncArgs {
                dry_run: true,
                force: false,
                verbose: true,
            },
        )),
        &json_ctx(),
    )
    .await;
    let _ = result;
}
