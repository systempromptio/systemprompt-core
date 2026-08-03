//! Tests for `ConfigSection` path resolution and the untyped YAML helpers
//! behind `admin config`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::str::FromStr;

use systemprompt_cli::admin::config::config_section::{
    ConfigSection, read_yaml_file, write_yaml_file,
};

fn services_root() -> std::path::PathBuf {
    systemprompt_test_fixtures::ensure_test_bootstrap()
        .services_path
        .clone()
}

#[test]
fn every_section_round_trips_through_its_name() {
    for section in ConfigSection::all() {
        let rendered = section.to_string();
        let parsed = ConfigSection::from_str(&rendered).unwrap();
        assert_eq!(parsed.to_string(), rendered);

        let upper = ConfigSection::from_str(&rendered.to_uppercase()).unwrap();
        assert_eq!(upper.to_string(), rendered);
    }
}

#[test]
fn an_unknown_section_name_is_rejected() {
    let err = ConfigSection::from_str("telemetry").unwrap_err();
    assert!(err.to_string().contains("telemetry"));
}

#[test]
fn each_section_resolves_a_file_under_the_profile() {
    let root = services_root();

    for section in ConfigSection::all() {
        let path = section.file_path().unwrap();
        if matches!(section, ConfigSection::Profile) {
            assert!(path.ends_with("profile.yaml"), "{}", path.display());
        } else {
            assert!(path.starts_with(&root), "{}", path.display());
        }
    }
}

#[test]
fn section_file_listings_walk_the_services_tree_recursively() {
    let root = services_root();
    let nested = root.join("skills/covnested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("config.yaml"), "id: covnested\n").unwrap();
    std::fs::write(nested.join("notes.txt"), "ignored").unwrap();

    let skills = ConfigSection::Skills.all_files().unwrap();
    assert!(
        skills.iter().any(|p| p.ends_with("covnested/config.yaml")),
        "{skills:?}"
    );
    assert!(
        skills
            .iter()
            .all(|p| { p.extension().is_some_and(|e| e == "yaml" || e == "yml") }),
        "{skills:?}"
    );

    let profile_files = ConfigSection::Profile.all_files().unwrap();
    assert_eq!(profile_files.len(), 1);
}

#[test]
fn a_section_whose_directory_is_absent_lists_nothing() {
    let root = services_root();
    let scheduler = root.join("scheduler");
    if scheduler.exists() {
        std::fs::remove_dir_all(&scheduler).unwrap();
    }

    assert!(ConfigSection::Scheduler.all_files().unwrap().is_empty());
}

#[test]
fn untyped_yaml_round_trips_unknown_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("doc.yaml");
    std::fs::write(
        &path,
        "known: 1\nunmodelled:\n  nested: keep-me\nlist:\n  - a\n  - b\n",
    )
    .unwrap();

    let mut doc = read_yaml_file(&path).unwrap();
    doc["known"] = serde_yaml::Value::from(2);
    write_yaml_file(&path, &doc).unwrap();

    let reread = read_yaml_file(&path).unwrap();
    assert_eq!(reread["known"], serde_yaml::Value::from(2));
    assert_eq!(reread["unmodelled"]["nested"].as_str(), Some("keep-me"));
    assert_eq!(reread["list"].as_sequence().unwrap().len(), 2);
}

#[test]
fn reading_reports_missing_files_and_parse_errors_with_the_path() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("absent.yaml");

    let err = read_yaml_file(&missing).unwrap_err();
    assert!(format!("{err:#}").contains("absent.yaml"));

    let broken = tmp.path().join("broken.yaml");
    std::fs::write(&broken, "key: [unterminated\n").unwrap();
    let err = read_yaml_file(&broken).unwrap_err();
    assert!(format!("{err:#}").contains("broken.yaml"));
}
