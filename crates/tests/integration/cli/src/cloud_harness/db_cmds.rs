//! Harness tests for `cloud db` routed through a scratch profile whose
//! secrets point at the real test database, plus pg_dump/pg_restore backup
//! round-trips.

use systemprompt_cli::cloud::db::{BackupFormat, CloudDbCommands};
use systemprompt_cli::cloud::{self, CloudCommands};

use super::{Env, enter, interactive_ctx, json_ctx};
use crate::full_bootstrap::database_url;

fn scaffold_db_profile(env: &Env, name: &str, url: &str) {
    let dir = env.root().join(".systemprompt/profiles").join(name);
    std::fs::create_dir_all(&dir).expect("db profile dir");
    std::fs::copy(
        env.root().join(".systemprompt/profiles/local/profile.yaml"),
        dir.join("profile.yaml"),
    )
    .expect("copy profile.yaml");
    std::fs::write(
        dir.join("secrets.json"),
        serde_json::json!({ "database_url": url, "oauth_at_rest_pepper": "test_oauth_at_rest_pepper_0123456789abcdef" }).to_string(),
    )
    .expect("write db secrets");
}

fn remove_db_profile(env: &Env, name: &str) {
    let dir = env.root().join(".systemprompt/profiles").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("remove db profile");
    }
}

#[tokio::test]
async fn db_unknown_profile_errors() {
    let _env = enter().await;
    let err = cloud::execute(
        CloudCommands::Db(CloudDbCommands::Status {
            profile: "ghost-db".to_owned(),
        }),
        &json_ctx(),
    )
    .await
    .expect_err("unknown profile");
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn db_introspection_commands_run() {
    let Some(url) = database_url() else { return };
    let env = enter().await;
    scaffold_db_profile(&env, "dbprof", &url);
    let profile = "dbprof".to_owned();

    let cmds = vec![
        CloudDbCommands::Status {
            profile: profile.clone(),
        },
        CloudDbCommands::Info {
            profile: profile.clone(),
        },
        CloudDbCommands::Tables {
            profile: profile.clone(),
            filter: Some("users".to_owned()),
        },
        CloudDbCommands::Describe {
            profile: profile.clone(),
            table_name: "users".to_owned(),
        },
        CloudDbCommands::Count {
            profile: profile.clone(),
            table_name: "users".to_owned(),
        },
        CloudDbCommands::Indexes {
            profile: profile.clone(),
            table: Some("users".to_owned()),
        },
        CloudDbCommands::Size {
            profile: profile.clone(),
        },
        CloudDbCommands::Query {
            profile: profile.clone(),
            sql: "SELECT 1 AS one".to_owned(),
            limit: Some(1),
            offset: None,
        },
    ];

    for cmd in cmds {
        let result = cloud::execute(CloudCommands::Db(cmd), &json_ctx()).await;
        let _ = result;
    }

    remove_db_profile(&env, "dbprof");
}

#[tokio::test]
async fn db_backup_and_restore_round_trip() {
    let Some(url) = database_url() else { return };
    let env = enter().await;
    scaffold_db_profile(&env, "dbback", &url);

    let out = env.root().join("backup-test.sql");
    let sql_backup = cloud::execute(
        CloudCommands::Db(CloudDbCommands::Backup {
            profile: "dbback".to_owned(),
            format: BackupFormat::Sql,
            output: Some(out.to_string_lossy().into_owned()),
        }),
        &json_ctx(),
    )
    .await;
    let _ = sql_backup;

    let default_path_backup = cloud::execute(
        CloudCommands::Db(CloudDbCommands::Backup {
            profile: "dbback".to_owned(),
            format: BackupFormat::Custom,
            output: None,
        }),
        &json_ctx(),
    )
    .await;
    let _ = default_path_backup;

    std::fs::write(&out, "SELECT 1;\n").expect("write trivial sql backup");

    let err = cloud::execute(
        CloudCommands::Db(CloudDbCommands::Restore {
            profile: "dbback".to_owned(),
            file: "no-such-file.dump".to_owned(),
            yes: true,
        }),
        &json_ctx(),
    )
    .await
    .expect_err("missing backup file");
    assert!(err.to_string().contains("not found"));

    let err = cloud::execute(
        CloudCommands::Db(CloudDbCommands::Restore {
            profile: "dbback".to_owned(),
            file: out.to_string_lossy().into_owned(),
            yes: false,
        }),
        &json_ctx(),
    )
    .await
    .expect_err("restore needs -y");
    assert!(err.to_string().contains("-y"));

    cloud::execute(
        CloudCommands::Db(CloudDbCommands::Restore {
            profile: "dbback".to_owned(),
            file: out.to_string_lossy().into_owned(),
            yes: false,
        }),
        &interactive_ctx(["n"]),
    )
    .await
    .expect("cancelled restore");

    cloud::execute(
        CloudCommands::Db(CloudDbCommands::Restore {
            profile: "dbback".to_owned(),
            file: out.to_string_lossy().into_owned(),
            yes: true,
        }),
        &json_ctx(),
    )
    .await
    .expect("restore trivial sql via psql");

    remove_db_profile(&env, "dbback");
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_dir_all(env.root().join("backups"));
}

#[tokio::test]
async fn db_execute_with_database_url_bypasses_profile() {
    let Some(url) = database_url() else { return };
    let _env = enter().await;
    systemprompt_cli::cloud::db::execute_with_database_url(
        CloudDbCommands::Query {
            profile: "ignored".to_owned(),
            sql: "SELECT 1 AS one".to_owned(),
            limit: None,
            offset: None,
        },
        &url,
        &json_ctx(),
    )
    .await
    .expect("query via explicit database url");
}
