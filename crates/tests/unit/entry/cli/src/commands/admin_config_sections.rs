//! Tests for `ConfigSection` path resolution and the untyped YAML helpers
//! behind `admin config`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::str::FromStr;

use systemprompt_cli::admin::config::config_section::{
    ConfigSection, GATEWAY_FILE_RELATIVE, GATEWAY_INCLUDE_RELATIVE, PROVIDERS_FILE_RELATIVE,
    PROVIDERS_INCLUDE_RELATIVE, read_yaml_file, write_yaml_file,
};
use systemprompt_cli::admin::config::services_io::append_include;

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

#[test]
fn providers_and_gateway_sections_resolve_the_ai_services_files() {
    let root = services_root();
    assert_eq!(
        ConfigSection::Providers.file_path().unwrap(),
        root.join("ai/providers.yaml")
    );
    assert_eq!(
        ConfigSection::Gateway.file_path().unwrap(),
        root.join("ai/gateway.yaml")
    );
}

#[test]
fn ai_listing_excludes_the_catalog_and_gateway_files() {
    let root = services_root();
    std::fs::create_dir_all(root.join("ai")).unwrap();
    std::fs::write(root.join("ai/config.yaml"), "ai: {}\n").unwrap();
    std::fs::write(root.join("ai/providers.yaml"), "providers: []\n").unwrap();
    std::fs::write(root.join("ai/gateway.yaml"), "gateway:\n  enabled: false\n").unwrap();

    let ai = ConfigSection::Ai.all_files().unwrap();
    assert!(ai.iter().any(|p| p.ends_with("ai/config.yaml")));
    assert!(!ai.iter().any(|p| p.ends_with("ai/providers.yaml")));
    assert!(!ai.iter().any(|p| p.ends_with("ai/gateway.yaml")));
    assert_eq!(
        ConfigSection::Providers.all_files().unwrap(),
        vec![root.join("ai/providers.yaml")]
    );
}

#[test]
fn append_include_splices_under_existing_includes_and_never_duplicates() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    std::fs::write(
        &root,
        "# root aggregator\nincludes:\n  - ../agents/a.yaml\nsettings:\n  x: 1\n",
    )
    .unwrap();

    append_include(&root, "ai/providers.yaml").unwrap();
    append_include(&root, "ai/providers.yaml").unwrap();

    let text = std::fs::read_to_string(&root).unwrap();
    assert_eq!(
        text,
        "# root aggregator\nincludes:\n  - ai/providers.yaml\n  - ../agents/a.yaml\nsettings:\n  x: 1\n"
    );
}

#[test]
fn append_include_creates_the_includes_list_when_absent() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("config.yaml");
    std::fs::write(&root, "settings:\n  x: 1\n").unwrap();

    append_include(&root, "ai/gateway.yaml").unwrap();

    let text = std::fs::read_to_string(&root).unwrap();
    assert_eq!(text, "settings:\n  x: 1\nincludes:\n  - ai/gateway.yaml\n");
}

#[test]
fn the_catalog_include_text_resolves_to_the_file_the_writer_creates() {
    let services = services_root();
    let config_dir = services.join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir");

    for (file_relative, include_relative) in [
        (PROVIDERS_FILE_RELATIVE, PROVIDERS_INCLUDE_RELATIVE),
        (GATEWAY_FILE_RELATIVE, GATEWAY_INCLUDE_RELATIVE),
    ] {
        let written = services.join(file_relative);
        std::fs::create_dir_all(written.parent().expect("parent")).expect("catalog dir");
        std::fs::write(&written, "{}\n").expect("write catalog file");

        let resolved = config_dir.join(include_relative);

        let resolved_real = std::fs::canonicalize(&resolved).unwrap_or_else(|e| {
            panic!(
                "`includes:` entries resolve against {}, so {include_relative} must reach {} \
                 — it resolved to {} ({e})",
                config_dir.display(),
                written.display(),
                resolved.display()
            )
        });
        let written_real = std::fs::canonicalize(&written).expect("written file canonicalises");
        assert_eq!(resolved_real, written_real);
    }
}

#[test]
fn an_includes_key_ending_the_file_still_gets_its_entry_on_a_new_line() {
    let dir = tempfile::tempdir().expect("tempdir");

    for content in ["settings:\n  x: 1\nincludes:", "includes:"] {
        let root = dir.path().join("config.yaml");
        std::fs::write(&root, content).expect("seed root");
        append_include(&root, "../ai/providers.yaml").expect("append");

        let updated = std::fs::read_to_string(&root).expect("read back");
        let doc: serde_yaml::Value = serde_yaml::from_str(&updated)
            .unwrap_or_else(|e| panic!("{updated:?} must stay parseable YAML: {e}"));
        let includes = doc
            .get("includes")
            .and_then(serde_yaml::Value::as_sequence)
            .unwrap_or_else(|| panic!("{updated:?} must keep `includes` a sequence"));
        assert_eq!(
            includes.first().and_then(serde_yaml::Value::as_str),
            Some("../ai/providers.yaml")
        );
    }
}
