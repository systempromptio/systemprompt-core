//! `admin config secret check` resolving a real profile's secrets source.
//!
//! The command is run when a boot has already failed, so it has to name the
//! source it would have read and the required keys that are absent, without
//! ever printing a value.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::secret_check::{execute, file_key_names, run};
use systemprompt_cli::cli_settings::{CliConfig, OutputFormat};
use systemprompt_config::ProfileBootstrap;

use crate::services_profile_fixture as fx;

const PEPPER: &str = "test_oauth_at_rest_pepper_for_secret_check_flows";
const SECRET_VALUE: &str = "sk-do-not-print-this";

fn install(secrets_section: &str, secrets_body: Option<&str>) -> fx::ProfileTree {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), secrets_section);
    if let Some(body) = secrets_body {
        std::fs::write(tree.root.join("secrets.json"), body).expect("write secrets.json");
    }
    ProfileBootstrap::init_from_path(&tree.profile_path).expect("profile installs");
    tree
}

fn secrets_json() -> String {
    format!(
        "{{\n  \"oauth_at_rest_pepper\": \"{PEPPER}\",\n  \"database_url\": \
         \"postgres://u:p@localhost/db\",\n  \"anthropic\": \"{SECRET_VALUE}\"\n}}\n"
    )
}

#[tokio::test]
async fn a_file_source_lists_its_key_names_and_reports_no_gaps() {
    let _tree = install(
        "secrets:\n  secrets_path: secrets.json\n  source: file\n",
        Some(&secrets_json()),
    );

    let report = run().await.expect("the file source resolves");
    assert_eq!(report.source, "file");
    assert!(report.detail.ends_with("secrets.json"), "{}", report.detail);
    assert_eq!(
        report.keys,
        vec!["anthropic", "database_url", "oauth_at_rest_pepper"]
    );
    assert!(report.missing_required.is_empty());
    let body = serde_json::to_string(&report).expect("serialises");
    assert!(!body.contains(SECRET_VALUE), "a value leaked: {body}");
}

#[tokio::test]
async fn a_file_source_missing_the_pepper_reports_the_gap() {
    let _tree = install(
        "secrets:\n  secrets_path: secrets.json\n  source: file\n",
        Some("{\n  \"database_url\": \"postgres://u:p@localhost/db\"\n}\n"),
    );

    let report = run().await.expect("the file source resolves");
    assert_eq!(report.missing_required, vec!["oauth_at_rest_pepper"]);
}

#[tokio::test]
async fn a_secrets_document_that_is_absent_names_the_path_it_looked_for() {
    let _tree = install(
        "secrets:\n  secrets_path: secrets.json\n  source: file\n",
        None,
    );

    let error = run().await.expect_err("a missing document is an error");
    assert!(format!("{error:#}").contains("secrets.json"), "{error:#}");
}

#[tokio::test]
async fn a_subprocess_reports_the_inherited_environment_and_its_present_keys() {
    fx::set_env("SYSTEMPROMPT_SUBPROCESS", "1");
    fx::set_env("OAUTH_AT_REST_PEPPER", PEPPER);
    fx::set_env("DATABASE_URL", "postgres://u:p@localhost/db");
    let _tree = install(
        "secrets:\n  secrets_path: secrets.json\n  source: env\n",
        Some(&secrets_json()),
    );

    let report = run().await.expect("the subprocess source resolves");
    assert_eq!(report.source, "subprocess-env");
    assert_eq!(report.detail, "inherited from the parent process");
    assert_eq!(report.keys, vec!["database_url", "oauth_at_rest_pepper"]);
    assert!(report.missing_required.is_empty());
}

#[tokio::test]
async fn the_command_renders_the_report_it_resolved() {
    let _tree = install(
        "secrets:\n  secrets_path: secrets.json\n  source: file\n",
        Some(&secrets_json()),
    );

    execute(
        &CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
    )
    .await
    .expect("the command renders");
}

#[test]
fn a_document_that_is_not_json_names_the_file_it_failed_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(&path, "not json at all").expect("write");
    let error = file_key_names(&path).expect_err("a non-JSON document is an error");
    assert!(format!("{error:#}").contains("secrets.json"), "{error:#}");
}

#[test]
fn a_json_document_that_is_not_an_object_has_no_keys() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(&path, "[\"a\", \"b\"]").expect("write");
    assert!(file_key_names(&path).expect("array parses").is_empty());
}
