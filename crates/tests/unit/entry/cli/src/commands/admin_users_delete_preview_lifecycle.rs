//! Public user deletion preview projects owned rows without mutating either
//! user.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
use systemprompt_cli::session::api::create_local_session_row;
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context, install_test_signing_key,
};
use systemprompt_users::{UserRepository, UserService};

const HELPER: &str = "commands::admin_users_delete_preview_lifecycle::delete_preview_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: UsersCommands,
}
fn parse(args: &[&str]) -> UsersCommands {
    Harness::try_parse_from(std::iter::once("users").chain(args.iter().copied()))
        .expect("parse users")
        .command
}

#[tokio::test]
#[ignore = "re-executed by deletion_preview_reports_owned_rows_and_preserves_target_and_sibling"]
async fn delete_preview_helper() {
    let database = DisposableDb::installed("cli_user_delete_preview")
        .await
        .expect("private database");
    // SAFETY: the ignored helper is process-isolated and configuration is not
    // initialized.
    unsafe {
        std::env::set_var("DATABASE_URL", database.url());
        std::env::set_var("TEST_DATABASE_URL", database.url());
    }
    ensure_test_bootstrap();
    install_test_signing_key();
    let pool = database.pool().await.expect("private pool");
    let service = UserService::new(Arc::new(
        UserRepository::new(&pool).expect("user repository"),
    ));
    let target = service
        .create(
            "preview-target",
            "preview-target@example.invalid",
            None,
            None,
        )
        .await
        .expect("target user");
    let sibling = service
        .create(
            "preview-sibling",
            "preview-sibling@example.invalid",
            None,
            None,
        )
        .await
        .expect("sibling user");
    let session = create_local_session_row(&pool, &target.id, chrono::Duration::hours(1))
        .await
        .expect("target session");
    let raw = pool.pool_arc().expect("SQL pool");
    let users_before: Vec<(String, String, String, Vec<String>)> = sqlx::query_as(
        "SELECT id, name, status, roles FROM users WHERE id IN ($1, $2) ORDER BY id",
    )
    .bind(target.id.as_str())
    .bind(sibling.id.as_str())
    .fetch_all(raw.as_ref())
    .await
    .expect("users before preview");
    let session_owner_before: String =
        sqlx::query_scalar("SELECT user_id FROM user_sessions WHERE session_id = $1")
            .bind(session.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("session before preview");
    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(&pool, database.url()).expect("full app context"),
    );
    println!("BEGIN_PREVIEW");
    users::execute(
        parse(&["delete", target.id.as_str(), "--dry-run"]),
        &context,
    )
    .await
    .expect("public deletion preview");
    println!("END_PREVIEW");
    let users_after: Vec<(String, String, String, Vec<String>)> = sqlx::query_as(
        "SELECT id, name, status, roles FROM users WHERE id IN ($1, $2) ORDER BY id",
    )
    .bind(target.id.as_str())
    .bind(sibling.id.as_str())
    .fetch_all(raw.as_ref())
    .await
    .expect("users after preview");
    let session_owner_after: String =
        sqlx::query_scalar("SELECT user_id FROM user_sessions WHERE session_id = $1")
            .bind(session.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("session after preview");
    println!(
        "PREVIEW_STATE={}",
        serde_json::json!({"users_before": users_before, "users_after": users_after, "session_owner_before": session_owner_before, "session_owner_after": session_owner_after, "target": target.id.as_str(), "sibling": sibling.id.as_str()})
    );
    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let mut stdout = tempfile::NamedTempFile::new().expect("stdout");
    let mut stderr = tempfile::NamedTempFile::new().expect("stderr");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().expect("stdout writer")))
        .stderr(Stdio::from(stderr.reopen().expect("stderr writer")));
    let mut child = command.spawn().expect("spawn helper");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = child.try_wait().expect("poll helper") {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break child.wait().expect("reap helper");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let read = |f: &mut tempfile::NamedTempFile| {
        f.seek(SeekFrom::Start(0)).expect("rewind");
        let mut b = Vec::new();
        f.read_to_end(&mut b).expect("read");
        b
    };
    Output {
        status,
        stdout: read(&mut stdout),
        stderr: read(&mut stderr),
    }
}

#[test]
fn deletion_preview_reports_owned_rows_and_preserves_target_and_sibling() {
    let mut command = Command::new(std::env::current_exe().expect("unit binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    let sanitize = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .split_whitespace()
            .map(|word| {
                if word.contains("postgres://") || word.contains("postgresql://") {
                    "<redacted-database-url>"
                } else {
                    word
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert!(
        output.status.success(),
        "delete preview helper failed\nstdout: {}\nstderr: {}",
        sanitize(&output.stdout),
        sanitize(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let artifact: serde_json::Value = serde_json::from_str(
        stdout
            .split_once("BEGIN_PREVIEW\n")
            .and_then(|(_, t)| t.split_once("\nEND_PREVIEW"))
            .map(|(j, _)| j)
            .expect("preview markers"),
    )
    .expect("preview artifact");
    assert_eq!(artifact["artifact_type"], "table");
    let rows = artifact["items"].as_array().expect("preview rows");
    let sessions = rows
        .iter()
        .find(|r| r["owner"] == "systemprompt-core" && r["table"] == "user_sessions")
        .expect("session preview row");
    assert_eq!(sessions["rows"], 1);
    let state: serde_json::Value = serde_json::from_str(
        stdout
            .split_once("PREVIEW_STATE=")
            .map(|(_, t)| t.lines().next().expect("state line"))
            .expect("state marker"),
    )
    .expect("state JSON");
    assert_eq!(state["users_after"], state["users_before"], "{state}");
    assert_eq!(
        state["users_after"].as_array().expect("users remain").len(),
        2
    );
    assert_eq!(state["session_owner_after"], state["session_owner_before"]);
    assert_eq!(state["session_owner_after"], state["target"]);
    assert_ne!(state["target"], state["sibling"]);
}
