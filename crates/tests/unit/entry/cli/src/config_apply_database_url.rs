//! `config apply --database-url` against a database other than the profile's:
//! the planes are recorded as applied in the database named by the flag and
//! the profile's own database is left alone.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::apply::{ApplyArgs, execute};
use systemprompt_cli::cli_settings::{CliConfig, OutputFormat};
use systemprompt_cli::context::CommandContext;
use systemprompt_cli::env_overrides::EnvOverrides;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, seed_fixture_system_admin, test_database_url,
};

async fn applied_planes(url: &str) -> i64 {
    let pool = sqlx::PgPool::connect(url).await.expect("connect");
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM config_plane_applied")
        .fetch_one(&pool)
        .await
        .expect("count applied planes");
    pool.close().await;
    count
}

#[tokio::test]
async fn apply_with_a_database_url_writes_to_that_database_only() {
    ensure_test_bootstrap();
    let other = DisposableDb::with_schema("cfg_apply_db_url").await;
    seed_fixture_system_admin(&other.test_pool().await)
        .await
        .expect("seed the system admin in the overriding database");
    let profile_db = test_database_url();
    let before = applied_planes(&profile_db).await;
    assert_eq!(applied_planes(other.url()).await, 0);

    let ctx = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_database_url_override(Some(other.url().to_owned()));
    let args = ApplyArgs {
        planes: Vec::new(),
        reason: "database-url test".to_owned(),
        dry_run: false,
        wait_secs: 30,
    };
    execute(&args, &ctx)
        .await
        .expect("apply against the overriding database");

    assert!(
        applied_planes(other.url()).await > 0,
        "the apply recorded its planes in the database the flag named"
    );
    assert_eq!(
        applied_planes(&profile_db).await,
        before,
        "the profile's database was not touched"
    );
    other.drop_now().await;
}
