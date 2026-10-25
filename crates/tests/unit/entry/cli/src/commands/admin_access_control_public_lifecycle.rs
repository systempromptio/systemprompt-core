//! Public access-control export and lint against durable database state.

use std::process::Command;

use systemprompt_cli::admin::access_control::{
    self, AccessControlCommands, ExportYamlArgs, LintArgs,
};
use systemprompt_cli::{CliConfig, EnvOverrides, OutputFormat};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    DisposableDb, bounded_output, ensure_test_bootstrap, seed_user_row_with_roles,
};

const HELPER: &str = "commands::admin_access_control_public_lifecycle::access_control_helper";

#[tokio::test]
#[ignore = "re-executed by public_export_round_trips_roles_and_lint_reports_unreachable_entities"]
async fn access_control_helper() {
    let database = DisposableDb::with_schema("cli_access_control_public").await;
    // SAFETY: this ignored helper is process-isolated and configuration is not
    // initialized.
    unsafe {
        std::env::set_var("DATABASE_URL", database.url());
        std::env::set_var("TEST_DATABASE_URL", database.url());
    }
    ensure_test_bootstrap();
    let pool = database.test_pool().await;
    let raw = pool.pool();
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
    sqlx::query("INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) VALUES ('agent', 'true', false, 'code:owned')")
        .execute(raw.as_ref()).await.expect("seed governed entity");
    sqlx::query("INSERT INTO access_control_entities (entity_type, entity_id, default_included, source) VALUES ('agent', 'unreachable-owned', false, 'code:owned')")
        .execute(raw.as_ref()).await.expect("seed unreachable entity");
    sqlx::query("INSERT INTO access_control_rules (entity_type, entity_id, rule_type, rule_value, access, justification, source) VALUES ('agent', 'true', 'role', '123', 'allow', 'owned: role', 'code:owned')")
        .execute(raw.as_ref()).await.expect("seed role rule");
    sqlx::query("INSERT INTO access_control_rules (entity_type, entity_id, rule_type, rule_value, access, justification, source) VALUES ('agent', 'true', 'user', 'omitted-user', 'deny', NULL, 'console:admin')")
        .execute(raw.as_ref()).await.expect("seed omitted user rule");
    let context = crate::boot_ctx::new_with_boot(
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
    pool.write_pool().close().await;
    drop(pool);
    database.drop_now().await;
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
    let output = bounded_output(command, "admin access control public lifecycle");
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
        "Access control (paste into services/access-control/rules.yaml)"
    );
    let yaml = export["content"].as_str().expect("export YAML text");
    let snapshot: serde_yaml::Value = serde_yaml::from_str(yaml).expect("parse exported YAML");
    let entities = snapshot["entities"]
        .as_sequence()
        .expect("exported entities");
    assert_eq!(entities.len(), 1, "role rules only: {snapshot:?}");
    let owned = entities
        .iter()
        .find(|entity| entity["entity"] == "agent/true")
        .expect("owned entity");
    assert_eq!(owned["allow"]["role"][0], "123");
    assert_eq!(owned["why"], "owned: role");
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
