//! Public plugin-token admission and backing-session persistence.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use base64::Engine;
use clap::Parser;
use systemprompt_cli::admin::keys::{self, KeysCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_identifiers::UserId;
use systemprompt_models::Config;
use systemprompt_security::HookTokenValidator;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, install_test_signing_key, seed_user_row_with_roles,
};

const HELPER: &str = "commands::admin_plugin_token_lifecycle::plugin_token_helper";
const PLUGIN: &str = "owned-coverage-plugin";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: KeysCommands,
}

fn parse(args: &[&str]) -> KeysCommands {
    Harness::try_parse_from(std::iter::once("keys").chain(args.iter().copied()))
        .expect("parse keys command")
        .command
}

#[tokio::test]
#[ignore = "re-executed by plugin_token_requires_admin_and_persists_its_backing_session"]
async fn plugin_token_helper() {
    let database = DisposableDb::installed("cli_plugin_token")
        .await
        .expect("private plugin-token database");
    // SAFETY: nextest gives the ignored helper its own process and configuration is
    // not yet read.
    unsafe {
        std::env::set_var("DATABASE_URL", database.url());
        std::env::set_var("TEST_DATABASE_URL", database.url());
    }
    ensure_test_bootstrap();
    install_test_signing_key();
    let pool = database.pool().await.expect("private plugin-token pool");
    let admin = UserId::new(uuid::Uuid::new_v4().to_string());
    let member = UserId::new(uuid::Uuid::new_v4().to_string());
    let admin_email = "plugin-admin@example.invalid";
    let member_email = "plugin-member@example.invalid";
    seed_user_row_with_roles(&pool, &admin, admin_email, &["admin".to_owned()])
        .await
        .expect("seed admin token subject");
    seed_user_row_with_roles(&pool, &member, member_email, &["user".to_owned()])
        .await
        .expect("seed non-admin token subject");
    let context = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    );

    println!("BEGIN_PLUGIN_TOKEN");
    keys::execute(
        parse(&[
            "issue-plugin-token",
            "--email",
            admin_email,
            "--plugin-id",
            PLUGIN,
            "--duration-days",
            "7",
        ]),
        &context,
    )
    .await
    .expect("admin plugin token issuance");
    println!("END_PLUGIN_TOKEN");

    let before_refusal: i64 = sqlx::query_scalar("SELECT count(*) FROM user_sessions")
        .fetch_one(pool.pool_arc().expect("private SQL pool").as_ref())
        .await
        .expect("count sessions before refusal");
    let refusal = keys::execute(
        parse(&[
            "issue-plugin-token",
            "--email",
            member_email,
            "--plugin-id",
            PLUGIN,
            "--duration-days",
            "7",
        ]),
        &context,
    )
    .await
    .expect_err("non-admin token subject must be refused");
    let after_refusal: i64 = sqlx::query_scalar("SELECT count(*) FROM user_sessions")
        .fetch_one(pool.pool_arc().expect("private SQL pool").as_ref())
        .await
        .expect("count sessions after refusal");
    let sessions: Vec<(String, String)> = sqlx::query_as(
        "SELECT session_id, user_id FROM user_sessions ORDER BY started_at, session_id",
    )
    .fetch_all(pool.pool_arc().expect("private SQL pool").as_ref())
    .await
    .expect("read token backing sessions");
    println!(
        "PLUGIN_TOKEN_STATE={}",
        serde_json::json!({
            "before_refusal": before_refusal,
            "after_refusal": after_refusal,
            "sessions": sessions,
            "admin_id": admin.as_str(),
            "refusal": format!("{refusal:#}"),
        })
    );

    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let mut stdout = tempfile::NamedTempFile::new().expect("plugin-token stdout");
    let mut stderr = tempfile::NamedTempFile::new().expect("plugin-token stderr");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().expect("stdout writer")))
        .stderr(Stdio::from(stderr.reopen().expect("stderr writer")));
    let mut child = command.spawn().expect("spawn plugin-token helper");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll plugin-token helper") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break child.wait().expect("reap plugin-token helper");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let read = |file: &mut tempfile::NamedTempFile| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).expect("read capture");
        bytes
    };
    Output {
        status,
        stdout: read(&mut stdout),
        stderr: read(&mut stderr),
    }
}

#[test]
fn plugin_token_requires_admin_and_persists_its_backing_session() {
    ensure_test_bootstrap();
    install_test_signing_key();
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(output.status.success(), "plugin-token helper failed");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 plugin-token output");
    let artifact = stdout
        .split_once("BEGIN_PLUGIN_TOKEN\n")
        .and_then(|(_, tail)| tail.split_once("\nEND_PLUGIN_TOKEN"))
        .map(|(json, _)| json)
        .expect("plugin-token output markers");
    let artifact: serde_json::Value =
        serde_json::from_str(artifact).expect("plugin-token JSON artifact");
    assert_eq!(artifact["title"], "Plugin-scope JWT");
    let sections = artifact["sections"].as_array().expect("token sections");
    let field = |heading: &str| {
        sections
            .iter()
            .find(|section| section["heading"] == heading)
            .unwrap_or_else(|| panic!("missing plugin-token field {heading}"))["content"]
            .clone()
    };
    assert_eq!(field("plugin_id"), PLUGIN);
    assert_eq!(field("email"), "plugin-admin@example.invalid");
    assert_eq!(field("expires_in_days"), 7);
    let token = field("token").as_str().expect("token string").to_owned();
    let segments = token.split('.').collect::<Vec<_>>();
    assert_eq!(segments.len(), 3, "issued value must be a JWT");
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(segments[1])
        .expect("decode JWT claims");
    let claims: serde_json::Value = serde_json::from_slice(&claims).expect("JWT claims JSON");
    let validator = HookTokenValidator::new(
        Config::get()
            .expect("fixture configuration")
            .jwt_issuer
            .clone(),
    );
    let govern = validator
        .validate_govern(&token, Some(PLUGIN))
        .expect("issued token has a valid signature, issuer, audience, govern scope, and plugin");
    let track = validator
        .validate_track(&token, Some(PLUGIN))
        .expect("issued token has a valid signature, issuer, audience, track scope, and plugin");
    assert_eq!(claims["plugin_id"], PLUGIN);
    assert_eq!(claims["aud"], serde_json::json!(["hook", "plugin"]));
    assert_eq!(claims["email"], "plugin-admin@example.invalid");
    assert_eq!(claims["scope"], "hook:govern hook:track");
    assert_eq!(govern.plugin_id.as_str(), PLUGIN);
    assert_eq!(track.plugin_id.as_str(), PLUGIN);
    assert_eq!(govern.subject.as_str(), claims["sub"]);
    assert_eq!(track.subject.as_str(), claims["sub"]);
    assert_eq!(
        claims["exp"].as_i64().expect("integer expiry")
            - claims["iat"].as_i64().expect("integer issued-at"),
        7 * 24 * 60 * 60,
    );
    assert_eq!(claims["jti"], field("jti"));
    let state = stdout
        .split_once("PLUGIN_TOKEN_STATE=")
        .map(|(_, tail)| tail.lines().next().expect("state line"))
        .expect("plugin-token durable state");
    let state: serde_json::Value = serde_json::from_str(state).expect("state JSON");
    assert_eq!(state["before_refusal"], 1, "{state}");
    assert_eq!(state["after_refusal"], 1, "{state}");
    assert_eq!(state["sessions"].as_array().expect("session rows").len(), 1);
    assert_eq!(state["sessions"][0][1], state["admin_id"], "{state}");
    assert_eq!(claims["sub"], state["admin_id"], "{claims}");
    assert_eq!(claims["session_id"], state["sessions"][0][0], "{claims}");
    assert!(
        state["refusal"]
            .as_str()
            .is_some_and(|message| message.contains("is not an admin")),
        "{state}"
    );
}
