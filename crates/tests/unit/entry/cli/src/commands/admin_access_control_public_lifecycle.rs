//! Public access-control export and lint against durable database state.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use systemprompt_cli::admin::access_control::{
    self, AccessControlCommands, ExportYamlArgs, LintArgs,
};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, install_test_signing_key, seed_user_row_with_roles,
};

const HELPER: &str = "commands::admin_access_control_public_lifecycle::access_control_helper";

#[tokio::test]
#[ignore = "re-executed by public_export_round_trips_roles_and_lint_reports_unreachable_entities"]
async fn access_control_helper() {
    let database = DisposableDb::installed("cli_access_control_public")
        .await
        .expect("private access-control database");
    // SAFETY: this ignored helper is process-isolated and configuration is not
    // initialized.
    unsafe {
        std::env::set_var("DATABASE_URL", database.url());
        std::env::set_var("TEST_DATABASE_URL", database.url());
    }
    ensure_test_bootstrap();
    install_test_signing_key();
    let pool = database.pool().await.expect("private access-control pool");
    let raw = pool.pool_arc().expect("SQL pool");
    let admin = UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row_with_roles(
        &pool,
        &admin,
        "testadmin@localhost.localdomain",
        &["admin".to_owned()],
    )
    .await
    .expect("seed configured system admin");
    sqlx::query("UPDATE users SET name = 'testadmin' WHERE id = $1")
        .bind(admin.as_str())
        .execute(raw.as_ref())
        .await
        .expect("match configured system admin name");
    sqlx::query("DELETE FROM access_control_rules")
        .execute(raw.as_ref())
        .await
        .expect("clear rules");
    sqlx::query("DELETE FROM access_control_entities")
        .execute(raw.as_ref())
        .await
        .expect("clear catalog");
    sqlx::query("INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) VALUES ('agent', 'true', false, 'owned')")
        .execute(raw.as_ref()).await.expect("seed governed entity");
    sqlx::query("INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) VALUES ('agent', 'unreachable-owned', false, 'owned')")
        .execute(raw.as_ref()).await.expect("seed unreachable entity");
    sqlx::query("INSERT INTO access_control_rules (entity_type, entity_id, rule_type, rule_value, access, justification) VALUES ('agent', 'true', 'role', '123', 'allow', 'owned: role')")
        .execute(raw.as_ref()).await.expect("seed role rule");
    sqlx::query("INSERT INTO access_control_rules (entity_type, entity_id, rule_type, rule_value, access, justification) VALUES ('agent', 'true', 'user', 'omitted-user', 'deny', NULL)")
        .execute(raw.as_ref()).await.expect("seed omitted user rule");
    let context = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    );
    println!("BEGIN_EXPORT");
    access_control::execute(AccessControlCommands::ExportYaml(ExportYamlArgs), &context)
        .await
        .expect("public export command");
    println!("END_EXPORT");
    println!("BEGIN_LINT");
    let error = access_control::execute(AccessControlCommands::Lint(LintArgs), &context)
        .await
        .expect_err("lint findings require nonzero result");
    println!("END_LINT");
    println!("LINT_ERROR={}", serde_json::json!(format!("{error:#}")));
    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let mut stdout = tempfile::NamedTempFile::new().expect("stdout capture");
    let mut stderr = tempfile::NamedTempFile::new().expect("stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().expect("stdout writer")))
        .stderr(Stdio::from(stderr.reopen().expect("stderr writer")));
    let mut child = command.spawn().expect("spawn access-control helper");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll helper") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break child.wait().expect("reap timed-out helper");
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

fn marked<'a>(stdout: &'a str, begin: &str, end: &str) -> &'a str {
    stdout
        .split_once(begin)
        .and_then(|(_, tail)| tail.split_once(end))
        .map(|(body, _)| body.trim())
        .expect("marked artifact")
}

#[test]
fn public_export_round_trips_roles_and_lint_reports_unreachable_entities() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
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
        "access-control helper failed\nstdout: {}\nstderr: {}",
        sanitize(&output.stdout),
        sanitize(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 helper output");
    let export: serde_json::Value =
        serde_json::from_str(marked(&stdout, "BEGIN_EXPORT", "END_EXPORT"))
            .expect("export artifact");
    assert_eq!(export["artifact_type"], "text", "{export}");
    assert_eq!(export["x-artifact-type"], "text", "{export}");
    assert_eq!(
        export["title"],
        "Access-control baseline (paste into services/access-control YAML)"
    );
    let yaml = export["content"].as_str().expect("export YAML text");
    let snapshot: serde_yaml::Value = serde_yaml::from_str(yaml).expect("parse exported YAML");
    let rules = snapshot["rules"].as_sequence().expect("exported rules");
    assert_eq!(rules.len(), 1, "role rules only: {snapshot:?}");
    let owned = rules
        .iter()
        .find(|rule| rule["entity_id"] == "true")
        .expect("owned rule");
    assert_eq!(owned["roles"][0], "123");
    assert_eq!(owned["justification"], "owned: role");
    assert!(!yaml.contains("omitted-user"));
    let lint: serde_json::Value =
        serde_json::from_str(marked(&stdout, "BEGIN_LINT", "END_LINT")).expect("lint artifact");
    assert_eq!(lint["artifact_type"], "text", "{lint}");
    assert_eq!(lint["x-artifact-type"], "text", "{lint}");
    assert_eq!(lint["title"], "Access-control lint");
    let report = lint["content"].as_str().expect("lint report");
    assert_eq!(
        report,
        "FAIL — 0 unknown, 1 unreachable\n\n[agent]\n  UNREACHABLE  unreachable-owned (catalog row present, default_included=false, no grants)\n"
    );
    assert!(stdout.contains("access-control lint failed; see report above"));
}
