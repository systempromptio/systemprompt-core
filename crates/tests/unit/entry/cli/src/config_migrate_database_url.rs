//! `admin config migrate --services <dir> --database-url <url>` through the
//! admin dispatcher: a database-scoped context serves migrate, with and without
//! `--check`, while the other `admin config` commands stay refused.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::AdminCommands;
use systemprompt_cli::admin::config::ConfigCommands;
use systemprompt_cli::admin::config::migrate::MigrateArgs;
use systemprompt_cli::cli_settings::{CliConfig, OutputFormat};
use systemprompt_cli::context::CommandContext;
use systemprompt_cli::env_overrides::EnvOverrides;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{DisposableDb, ensure_test_bootstrap};

async fn scoped(db: &DisposableDb) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(db.test_pool().await),
        db.url().to_owned(),
    )
}

fn migrate(check: bool, services: &std::path::Path) -> AdminCommands {
    AdminCommands::Config(ConfigCommands::Migrate(MigrateArgs {
        check,
        data: false,
        services: Some(services.to_path_buf()),
        profiles: None,
    }))
}

#[tokio::test]
async fn migrate_and_migrate_check_run_against_a_database_url() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cfg_migrate_db_url").await;
    let ctx = scoped(&db).await;
    let services = tempfile::tempdir().expect("services dir");

    for check in [false, true] {
        systemprompt_cli::admin::execute(migrate(check, services.path()), &ctx)
            .await
            .unwrap_or_else(|e| panic!("migrate (check={check}) refused: {e:#}"));
    }
    db.drop_now().await;
}

#[tokio::test]
async fn other_config_commands_stay_refused_under_a_database_url() {
    ensure_test_bootstrap();
    let db = DisposableDb::with_schema("cfg_show_db_url").await;
    let ctx = scoped(&db).await;

    systemprompt_cli::admin::execute(AdminCommands::Config(ConfigCommands::Show), &ctx)
        .await
        .expect_err("config show needs the profile");
    db.drop_now().await;
}
