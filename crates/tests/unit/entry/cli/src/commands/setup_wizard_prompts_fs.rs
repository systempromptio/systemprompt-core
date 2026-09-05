//! The setup wizard's prompt-driven decisions.
//!
//! Each of these takes the answer from a flag when one is given, from the
//! prompter when the terminal is interactive, and from a default otherwise.
//! Only the flag and default arms had ever run; the prompted arms are driven
//! here through `ScriptedPrompter`, which needs no terminal.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::setup::SetupArgs;
use systemprompt_cli::admin::setup::wizard_prompts::{
    detect_project_root, get_environment_name, resolve_admin_email, should_run_migrations,
};
use systemprompt_cli::interactive::ScriptedPrompter;

fn args() -> SetupArgs {
    SetupArgs {
        environment: None,
        docker: false,
        db_host: "127.0.0.1".to_owned(),
        db_port: 5432,
        port_offset: 0,
        db_user: None,
        db_password: None,
        db_name: None,
        gemini_key: None,
        anthropic_key: None,
        openai_key: None,
        github_token: None,
        default_provider: None,
        admin_email: None,
        migrate: false,
        no_migrate: false,
        dry_run: false,
        yes: false,
        force: false,
    }
}

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn interactive() -> CliConfig {
    CliConfig::new()
        .with_interactive(true)
        .with_assume_terminal(true)
}

fn non_interactive() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

#[test]
fn an_environment_flag_wins_over_the_prompt() {
    let mut flagged = args();
    flagged.environment = Some("staging".to_owned());

    let name = get_environment_name(&flagged, &scripted(&[]), &interactive())
        .expect("the flag answers without asking");

    assert_eq!(name, "staging");
}

#[test]
fn a_prompted_environment_name_is_trimmed_and_lowercased() {
    let name = get_environment_name(&args(), &scripted(&["  PROD  "]), &interactive())
        .expect("a prompted name is accepted");

    assert_eq!(name, "prod");
}

#[test]
fn an_empty_answer_takes_the_offered_default() {
    let name = get_environment_name(&args(), &scripted(&[""]), &interactive())
        .expect("an empty answer falls back to the default");

    assert_eq!(name, "dev");
}

#[test]
fn without_a_terminal_the_environment_defaults_without_asking() {
    let name = get_environment_name(&args(), &scripted(&[]), &non_interactive())
        .expect("no prompt is made without a terminal");

    assert_eq!(name, "dev");
}

#[test]
fn an_admin_email_flag_is_validated_rather_than_trusted() {
    let mut flagged = args();
    flagged.admin_email = Some("  admin@example.test  ".to_owned());

    let email = resolve_admin_email(&flagged, &scripted(&[]), &non_interactive())
        .expect("a well-formed address passes");
    assert_eq!(email.as_str(), "admin@example.test");

    flagged.admin_email = Some("not-an-address".to_owned());
    let err = resolve_admin_email(&flagged, &scripted(&[]), &non_interactive())
        .expect_err("a malformed --admin-email must be refused, not stored");
    assert!(
        format!("{err:#}").contains("not a valid email address"),
        "the refusal must name the flag's value as the problem, got: {err:#}"
    );
}

#[test]
fn a_prompted_admin_email_is_accepted_and_an_empty_one_is_refused() {
    let email = resolve_admin_email(&args(), &scripted(&["  owner@example.test "]), &interactive())
        .expect("a prompted address is accepted");
    assert_eq!(email.as_str(), "owner@example.test");

    let err = resolve_admin_email(&args(), &scripted(&["   "]), &interactive())
        .expect_err("an empty answer must not become the admin identity");
    assert!(
        format!("{err:#}").contains("required"),
        "the refusal must say the address is required, got: {err:#}"
    );
}

// Why: this address ends up identifying the platform admin on sign-in and
// consent screens, so a missing one is a hard stop rather than a default.
#[test]
fn without_a_terminal_a_missing_admin_email_is_a_hard_stop() {
    let err = resolve_admin_email(&args(), &scripted(&[]), &non_interactive())
        .expect_err("setup must not invent an administrator");

    assert!(
        format!("{err:#}").contains("--admin-email"),
        "the refusal must name the flag that supplies it, got: {err:#}"
    );
}

#[test]
fn the_migration_flags_win_over_the_prompt_in_both_directions() {
    let mut forced = args();
    forced.migrate = true;
    assert!(
        should_run_migrations(&forced, &scripted(&[]), &interactive()).expect("--migrate"),
        "--migrate must not ask"
    );

    let mut refused = args();
    refused.no_migrate = true;
    assert!(
        !should_run_migrations(&refused, &scripted(&[]), &interactive()).expect("--no-migrate"),
        "--no-migrate must not ask"
    );
}

#[test]
fn a_prompted_migration_answer_decides_it() {
    assert!(
        should_run_migrations(&args(), &scripted(&["y"]), &interactive()).expect("accepted"),
        "an accepted prompt runs migrations"
    );
    assert!(
        !should_run_migrations(&args(), &scripted(&["n"]), &interactive()).expect("declined"),
        "a declined prompt does not"
    );
}

#[test]
fn without_a_terminal_migrations_are_not_run_unasked() {
    assert!(
        !should_run_migrations(&args(), &scripted(&[]), &non_interactive())
            .expect("no prompt without a terminal"),
        "an unattended setup must not migrate a database on its own"
    );
}

#[test]
fn the_project_root_is_the_nearest_directory_carrying_a_project_marker() {
    let root = detect_project_root().expect("a root is always resolved");

    assert!(
        root.join("Cargo.toml").exists()
            || root.join("services").exists()
            || root.join(".systemprompt").exists()
            || root.join("core").exists(),
        "the resolved root must carry one of the markers it searches for: {}",
        root.display()
    );
}
