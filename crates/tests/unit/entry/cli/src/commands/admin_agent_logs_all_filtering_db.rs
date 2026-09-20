#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::agents::{logs, logs_db};
use systemprompt_identifiers::{SessionId, TraceId, UserId};
use systemprompt_logging::{LogActor, LogEntry, LogLevel, LoggingRepository};
use systemprompt_test_fixtures::{DisposableDb, ensure_test_bootstrap, seed_user_row};

async fn seed(repository: &LoggingRepository, actor: &LogActor, module: &str, message: &str) {
    repository
        .log(LogEntry::new(
            LogLevel::Info,
            module,
            message,
            actor.clone(),
        ))
        .await
        .expect("seed log entry");
}

#[tokio::test]
async fn all_agent_database_logs_match_operational_modules_strip_ansi_and_hide_profile_noise() {
    ensure_test_bootstrap();
    let database = DisposableDb::installed("cli_agent_logs_all")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated pool");
    let user = UserId::new(format!("logs-all-{}", uuid::Uuid::new_v4().simple()));
    seed_user_row(&pool, &user, &format!("{}@logs.invalid", user.as_str()))
        .await
        .expect("seed log actor");
    let actor = LogActor::new(user, SessionId::generate(), TraceId::generate());
    let repository = LoggingRepository::new(&pool).expect("logging repository");
    seed(
        &repository,
        &actor,
        "a2a.dispatch",
        "\u{1b}[31mowned dispatch failed\u{1b}[0m",
    )
    .await;
    seed(
        &repository,
        &actor,
        "agent.orchestration",
        "[profile:internal] noisy startup",
    )
    .await;
    seed(
        &repository,
        &actor,
        "billing.worker",
        "unrelated billing log",
    )
    .await;

    let output = logs_db::execute_db_mode_with_pool(
        &logs::LogsArgs {
            agent: None,
            lines: 50,
            follow: false,
            disk: false,
            logs_dir: None,
        },
        &pool,
        &CliConfig::new().with_interactive(false),
    )
    .await
    .expect("query all agent database logs");
    let artifact = serde_json::to_value(output.artifact()).expect("serialize logs artifact");
    let rendered = artifact.to_string();
    assert_eq!(artifact["title"], "Agent Logs (DB): all");
    assert!(rendered.contains("owned dispatch failed"), "{artifact}");
    assert!(!rendered.contains("\\u001b"), "{artifact}");
    assert!(!rendered.contains("noisy startup"), "{artifact}");
    assert!(!rendered.contains("unrelated billing log"), "{artifact}");

    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
