//! Tests for small public helpers that no other test names.
//!
//! Each is a pure projection or a file writer taking an explicit path, so they
//! are testable directly rather than through their commands.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::EnvOverrides;
use systemprompt_cli::admin::setup::SetupArgs;
use systemprompt_cli::core::skills::types::parse_skill_from_config;

#[derive(Debug, Parser)]
struct SetupHarness {
    #[command(flatten)]
    args: SetupArgs,
}

fn setup_args(extra: &[&str]) -> SetupArgs {
    SetupHarness::try_parse_from(std::iter::once("setup").chain(extra.iter().copied()))
        .unwrap()
        .args
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
