//! Tests for the small public `cloud profile` helpers that no other test
//! names: API-key normalisation on `CreateArgs` and the profile and Dockerfile
//! writers, each taking an explicit path.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use clap::Parser;
use systemprompt_cloud_entry::cli::profile::CreateArgs;
use systemprompt_cloud_entry::cli::profile::templates::{save_dockerfile, save_profile};
use systemprompt_manifest::Profile;

#[derive(Debug, Parser)]
struct CreateHarness {
    #[command(flatten)]
    args: CreateArgs,
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
        &pname("covprofile"),
        Path::new("/var/www/html/systemprompt-core"),
    )
    .unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("covprofile"), "{written}");
    assert!(written.contains("FROM"), "{written}");
}

fn pname(name: &str) -> systemprompt_identifiers::ProfileName {
    systemprompt_identifiers::ProfileName::try_new(name).expect("valid ProfileName")
}
