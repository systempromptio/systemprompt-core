//! Tests for small public helpers that no other test names.
//!
//! Each is a pure projection or a file writer taking an explicit path, so they
//! are testable directly rather than through their commands.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use clap::Parser;
use systemprompt_cli::EnvOverrides;
use systemprompt_cli::admin::setup::SetupArgs;
use systemprompt_cli::cloud::profile::CreateArgs;
use systemprompt_cli::cloud::profile::templates::{save_dockerfile, save_profile};
use systemprompt_cli::core::skills::types::parse_skill_from_config;
use systemprompt_models::Profile;

#[derive(Debug, Parser)]
struct SetupHarness {
    #[command(flatten)]
    args: SetupArgs,
}

#[derive(Debug, Parser)]
struct CreateHarness {
    #[command(flatten)]
    args: CreateArgs,
}

fn setup_args(extra: &[&str]) -> SetupArgs {
    SetupHarness::try_parse_from(std::iter::once("setup").chain(extra.iter().copied()))
        .unwrap()
        .args
}

fn create_args(extra: &[&str]) -> CreateArgs {
    CreateHarness::try_parse_from(std::iter::once("create").chain(extra.iter().copied()))
        .unwrap()
        .args
}

fn fixture_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    serde_yaml::from_str(&std::fs::read_to_string(&boot.profile_path).unwrap()).unwrap()
}

#[test]
fn database_identifiers_default_to_the_environment_name() {
    let args = setup_args(&[]);

    assert_eq!(args.effective_db_user("staging"), "systemprompt_staging");
    assert_eq!(args.effective_db_name("staging"), "systemprompt_staging");
}

#[test]
fn explicit_database_identifiers_win_over_the_environment_default() {
    let args = setup_args(&["--db-user", "custom_user", "--db-name", "custom_db"]);

    assert_eq!(args.effective_db_user("staging"), "custom_user");
    assert_eq!(args.effective_db_name("staging"), "custom_db");
}

#[test]
fn blank_api_keys_are_normalised_away() {
    let args = create_args(&[
        "covprofile",
        "--anthropic-key",
        "   ",
        "--openai-key",
        "sk-real",
        "--gemini-key",
        "",
    ])
    .normalized();

    assert!(args.anthropic_key.is_none());
    assert!(args.gemini_key.is_none());
    assert_eq!(args.openai_key.as_deref(), Some("sk-real"));
    assert!(args.has_api_key());
}

#[test]
fn a_wholly_blank_key_set_normalises_to_no_keys() {
    let args =
        create_args(&["covprofile", "--anthropic-key", "", "--openai-key", "  "]).normalized();

    assert!(!args.has_api_key());
}

#[test]
fn a_skill_is_parsed_from_its_config_and_content_file() {
    let dir = tempfile::tempdir().unwrap();
    let skill = dir.path().join("covskill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: covskill\nname: Coverage Skill\ndescription: Parsed from disk\nenabled: \
         true\ntags:\n  - alpha\n  - beta\ncategory: tools\n",
    )
    .unwrap();
    std::fs::write(
        skill.join("index.md"),
        "---\ntitle: ignored frontmatter\n---\n\nThe real instructions.\n",
    )
    .unwrap();

    let parsed = parse_skill_from_config(&skill.join("config.yaml"), &skill).unwrap();

    assert_eq!(parsed.name, "Coverage Skill");
    assert_eq!(parsed.description, "Parsed from disk");
    assert!(parsed.enabled);
    assert_eq!(parsed.tags, vec!["alpha".to_owned(), "beta".to_owned()]);
    assert_eq!(parsed.category.as_deref(), Some("tools"));
    assert!(parsed.instructions.contains("The real instructions."));
    assert!(!parsed.instructions.contains("ignored frontmatter"));
}

#[test]
fn a_skill_without_a_content_file_parses_with_empty_instructions() {
    let dir = tempfile::tempdir().unwrap();
    let skill = dir.path().join("bare");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: bare\nname: Bare Skill\ndescription: No content file\n",
    )
    .unwrap();

    let parsed = parse_skill_from_config(&skill.join("config.yaml"), &skill).unwrap();
    assert_eq!(parsed.name, "Bare Skill");
    assert!(parsed.instructions.is_empty());
}

#[test]
fn an_unreadable_or_invalid_skill_config_names_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("absent/config.yaml");
    let err = parse_skill_from_config(&missing, dir.path()).unwrap_err();
    assert!(format!("{err:#}").contains("config.yaml"));

    let broken = dir.path().join("config.yaml");
    std::fs::write(&broken, "name: [unterminated\n").unwrap();
    let err = parse_skill_from_config(&broken, dir.path()).unwrap_err();
    assert!(format!("{err:#}").contains("config.yaml"));
}

#[test]
fn a_profile_is_saved_with_a_generated_header() {
    let profile = fixture_profile();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/profile.yaml");

    save_profile(&profile, &path).unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.starts_with("# systemprompt.io Profile:"));
    assert!(written.contains(&profile.display_name));

    let reparsed: Profile = serde_yaml::from_str(&written).unwrap();
    assert_eq!(reparsed.name, profile.name);
}

#[test]
fn a_dockerfile_is_written_for_the_named_profile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Dockerfile");

    save_dockerfile(
        &path,
        "covprofile",
        Path::new("/var/www/html/systemprompt-core"),
    )
    .unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("covprofile"), "{written}");
    assert!(written.contains("FROM"), "{written}");
}

#[test]
fn process_environment_overrides_are_read_without_panicking() {
    let from_process = EnvOverrides::from_process_env();
    let from_vars = EnvOverrides::from_vars([
        ("SYSTEMPROMPT_PROFILE", "/tmp/cov-profile.yaml"),
        ("NO_COLOR", "1"),
    ]);

    assert_eq!(
        from_vars.profile.as_deref(),
        Some("/tmp/cov-profile.yaml"),
        "explicit vars are honoured"
    );
    assert!(from_vars.no_color);
    // The ambient environment decides these; only their agreement is assertable.
    assert_eq!(
        from_process.profile.is_some(),
        std::env::var("SYSTEMPROMPT_PROFILE").is_ok()
    );
}
