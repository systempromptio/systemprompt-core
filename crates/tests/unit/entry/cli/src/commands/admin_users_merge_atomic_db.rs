#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_identifiers::SessionId;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context, install_test_signing_key,
    seed_user_session,
};
use systemprompt_users::{UserRepository, UserService};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: UsersCommands,
}

fn parse(args: &[&str]) -> UsersCommands {
    Harness::try_parse_from(std::iter::once("users").chain(args.iter().copied()))
        .expect("parse users command")
        .command
}

#[tokio::test]
async fn merge_rolls_back_transfers_on_late_failure_and_retry_commits_once() {
    let database = DisposableDb::installed("cli_user_merge_atomic")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    ensure_test_bootstrap();
    install_test_signing_key();
    let repository = Arc::new(UserRepository::new(&pool).expect("user repository"));
    let service = UserService::new(Arc::clone(&repository));
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let source = service
        .create(
            &format!("merge_source_{nonce}"),
            &format!("source-{nonce}@merge.invalid"),
            None,
            None,
        )
        .await
        .expect("source user");
    let target = service
        .create(
            &format!("merge_target_{nonce}"),
            &format!("target-{nonce}@merge.invalid"),
            None,
            None,
        )
        .await
        .expect("target user");
    let session = SessionId::generate();
    seed_user_session(&pool, &source.id, &session)
        .await
        .expect("source session");
    let raw = pool.pool_arc().expect("raw pool");
    sqlx::query(
        "CREATE FUNCTION reject_cli_merge_attribution() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN IF NEW.tool_name = 'users.merge' THEN RAISE EXCEPTION 'fixture late merge failure'; \
         END IF; RETURN NEW; END $$",
    )
    .execute(raw.as_ref())
    .await
    .expect("create failure function");
    sqlx::query(
        "CREATE TRIGGER reject_cli_merge_attribution BEFORE INSERT ON governance_decisions FOR \
         EACH ROW EXECUTE FUNCTION reject_cli_merge_attribution()",
    )
    .execute(raw.as_ref())
    .await
    .expect("create late failure trigger");

    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(&pool, database.url()).expect("isolated full app context"),
    );
    let command = || {
        parse(&[
            "merge",
            "--source",
            source.id.as_str(),
            "--target",
            target.id.as_str(),
            "--yes",
        ])
    };

    let error = users::execute(command(), &context)
        .await
        .expect_err("late attribution failure must reject the merge");
    assert!(
        format!("{error:#}").contains("fixture late merge failure"),
        "unexpected merge failure: {error:#}"
    );
    assert!(
        service.find_by_id(&source.id).await.unwrap().is_some(),
        "rollback must preserve the source user"
    );
    assert!(service.find_by_id(&target.id).await.unwrap().is_some());
    let owner: String =
        sqlx::query_scalar("SELECT user_id FROM user_sessions WHERE session_id = $1")
            .bind(session.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("session after rollback");
    assert_eq!(owner, source.id.as_str(), "session transfer must roll back");
    let attribution_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM governance_decisions WHERE tool_name = 'users.merge'",
    )
    .fetch_one(raw.as_ref())
    .await
    .expect("attribution count after rollback");
    assert_eq!(
        attribution_count, 0,
        "failed merge must leave no audit claim"
    );

    sqlx::query("DROP TRIGGER reject_cli_merge_attribution ON governance_decisions")
        .execute(raw.as_ref())
        .await
        .expect("remove failure trigger");
    sqlx::query("DROP FUNCTION reject_cli_merge_attribution()")
        .execute(raw.as_ref())
        .await
        .expect("remove failure function");
    users::execute(command(), &context)
        .await
        .expect("retry succeeds after the persistence fault is repaired");

    assert!(
        service.find_by_id(&source.id).await.unwrap().is_none(),
        "successful retry deletes the source"
    );
    let owner: String =
        sqlx::query_scalar("SELECT user_id FROM user_sessions WHERE session_id = $1")
            .bind(session.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("session after retry");
    assert_eq!(
        owner,
        target.id.as_str(),
        "retry transfers the session once"
    );
    let attribution_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM governance_decisions WHERE tool_name = 'users.merge'",
    )
    .fetch_one(raw.as_ref())
    .await
    .expect("attribution count after retry");
    assert_eq!(attribution_count, 1, "retry records one merge attribution");

    drop(context);
    drop(service);
    drop(repository);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
