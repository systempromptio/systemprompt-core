#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context, install_test_signing_key,
    seed_user_row,
};

const HELPER: &str = "commands::core_files_ai_output_lifecycle::files_ai_output_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: CoreCommands,
}

fn parse(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
        .expect("parse core files AI command")
        .command
}

#[tokio::test]
#[ignore = "re-executed by public_ai_file_commands_render_the_same_isolated_image"]
async fn files_ai_output_helper() {
    let database = DisposableDb::installed("cli_files_ai_output")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated database pool");
    ensure_test_bootstrap();
    install_test_signing_key();
    let user = UserId::new(format!("ai-output-{}", uuid::Uuid::new_v4().simple()));
    seed_user_row(&pool, &user, &format!("{}@files.invalid", user.as_str()))
        .await
        .expect("seed image owner");
    let other_user = UserId::new(format!("ai-output-other-{}", uuid::Uuid::new_v4().simple()));
    seed_user_row(
        &pool,
        &other_user,
        &format!("{}@files.invalid", other_user.as_str()),
    )
    .await
    .expect("seed other image owner");
    let id = uuid::Uuid::new_v4();
    let path = format!("/owned/generated/{id}.png");
    let raw = pool.pool_arc().expect("raw pool");
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content, user_id, \
         metadata) VALUES ($1, $2, $3, 'image/png', 73, true, $4, $5)",
    )
    .bind(id)
    .bind(&path)
    .bind(format!("https://files.invalid/{id}.png"))
    .bind(user.as_str())
    .bind(serde_json::json!({
        "checksums": {"sha256": "fixture-sha256"},
        "type_specific": {
            "type": "image",
            "width": 640,
            "height": 480,
            "alt_text": "generated fixture"
        }
    }))
    .execute(raw.as_ref())
    .await
    .expect("seed AI image");
    let non_ai_id = uuid::Uuid::new_v4();
    let other_ai_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content, user_id, metadata) \
         VALUES ($1, $2, $2, 'image/png', 12, false, $3, '{}'::jsonb), \
                ($4, $5, $5, 'image/png', 24, true, $6, '{}'::jsonb)",
    )
    .bind(non_ai_id)
    .bind(format!("/owned/manual/{non_ai_id}.png"))
    .bind(user.as_str())
    .bind(other_ai_id)
    .bind(format!("/owned/generated/{other_ai_id}.png"))
    .bind(other_user.as_str())
    .execute(raw.as_ref())
    .await
    .expect("seed filtering controls");

    let context = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(&pool, database.url()).expect("isolated app context"),
    );
    println!("BEGIN_AI_LIST");
    core::execute(
        parse(&[
            "files",
            "ai",
            "list",
            "--user",
            user.as_str(),
            "--limit",
            "10",
        ]),
        &context,
    )
    .await
    .expect("public AI list");
    println!("END_AI_LIST");
    println!("BEGIN_AI_SHOW");
    core::execute(parse(&["files", "ai", "show", &id.to_string()]), &context)
        .await
        .expect("public AI show");
    println!("END_AI_SHOW");
    println!("BEGIN_AI_COUNT");
    core::execute(
        parse(&["files", "ai", "count", "--user", user.as_str()]),
        &context,
    )
    .await
    .expect("public AI count");
    println!("END_AI_COUNT");
    println!("FIXTURE_ID={id}");
    println!("FIXTURE_PATH={path}");
    println!("NON_AI_ID={non_ai_id}");
    println!("OTHER_AI_ID={other_ai_id}");

    drop(context);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("AI output stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("AI output stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn AI output helper");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().expect("poll AI output helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap AI output helper");
            panic!(
                "AI output helper timed out ({status})\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&std::fs::read(stdout.path()).unwrap()),
                String::from_utf8_lossy(&std::fs::read(stderr.path()).unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn marked_json(stdout: &str, start: &str, end: &str) -> serde_json::Value {
    let json = stdout
        .split_once(start)
        .and_then(|(_, tail)| tail.split_once(end))
        .map(|(value, _)| value.trim())
        .unwrap_or_else(|| panic!("missing {start}/{end} in {stdout}"));
    serde_json::from_str(json).unwrap_or_else(|error| panic!("invalid JSON: {error}: {json}"))
}

fn section<'a>(artifact: &'a serde_json::Value, heading: &str) -> &'a serde_json::Value {
    &artifact["sections"]
        .as_array()
        .expect("card sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
}

#[test]
fn public_ai_file_commands_render_the_same_isolated_image() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "AI output helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("AI output UTF-8");
    let id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("FIXTURE_ID="))
        .expect("fixture id marker");
    let path = stdout
        .lines()
        .find_map(|line| line.strip_prefix("FIXTURE_PATH="))
        .expect("fixture path marker");
    let non_ai_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("NON_AI_ID="))
        .expect("non-AI control marker");
    let other_ai_id = stdout
        .lines()
        .find_map(|line| line.strip_prefix("OTHER_AI_ID="))
        .expect("other-owner control marker");
    let list = marked_json(&stdout, "BEGIN_AI_LIST", "END_AI_LIST");
    let show = marked_json(&stdout, "BEGIN_AI_SHOW", "END_AI_SHOW");
    let count = marked_json(&stdout, "BEGIN_AI_COUNT", "END_AI_COUNT");

    assert_eq!(list["artifact_type"], "table", "{list}");
    assert!(
        list["columns"]
            .as_array()
            .is_some_and(|columns| !columns.is_empty()),
        "{list}"
    );
    assert_eq!(list["items"].as_array().map(Vec::len), Some(1), "{list}");
    assert_eq!(list["items"][0]["id"], id, "{list}");
    assert_eq!(list["items"][0]["path"], path, "{list}");
    assert!(!list.to_string().contains(non_ai_id), "{list}");
    assert!(!list.to_string().contains(other_ai_id), "{list}");
    assert_eq!(show["title"], format!("AI Image: {id}"), "{show}");
    assert_eq!(section(&show, "id"), id, "{show}");
    assert_eq!(section(&show, "path"), path, "{show}");
    assert_eq!(section(&show, "ai_content"), true, "{show}");
    assert_eq!(section(&show, "metadata")["image"]["width"], 640, "{show}");
    assert_eq!(section(&count, "count"), 1, "{count}");
}
