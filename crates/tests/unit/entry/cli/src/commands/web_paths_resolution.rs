//! Tests for web template/asset path resolution.
//!
//! `WebPaths::resolve_from_profile` reads the profile's web config and
//! normalises both entries under the services root; the defaults, the
//! configured overrides, and the absolute-path escape hatch are all distinct
//! branches.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_cli::web::paths::WebPaths;
use systemprompt_models::Profile;

fn fixture() -> (Profile, PathBuf) {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let profile: Profile =
        serde_yaml::from_str(&std::fs::read_to_string(&boot.profile_path).unwrap()).unwrap();
    let services = PathBuf::from(&profile.paths.services);
    (profile, services)
}

fn write_web_config(services: &std::path::Path, yaml: &str) {
    let dir = services.join("web");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.yaml"), yaml).unwrap();
}

#[test]
fn a_config_without_a_paths_block_falls_back_to_the_defaults() {
    let (profile, services) = fixture();
    write_web_config(&services, "{}\n");

    let paths = WebPaths::resolve_from_profile(&profile).unwrap();
    assert_eq!(paths.templates, services.join("web/templates"));
    assert_eq!(paths.assets, services.join("web/assets"));
}

#[test]
fn configured_relative_paths_are_normalised_under_the_services_root() {
    let (profile, services) = fixture();
    write_web_config(
        &services,
        "paths:\n  templates: services/web/custom-templates\n  assets: web/custom-assets\n",
    );

    let paths = WebPaths::resolve_from_profile(&profile).unwrap();
    assert_eq!(paths.templates, services.join("web/custom-templates"));
    assert_eq!(paths.assets, services.join("web/custom-assets"));
}

#[test]
fn an_absolute_configured_path_is_used_verbatim() {
    let (profile, services) = fixture();
    let tmp = tempfile::tempdir().unwrap();
    let absolute = tmp.path().join("shared-templates");
    write_web_config(
        &services,
        &format!(
            "paths:\n  templates: {}\n  assets: web/assets\n",
            absolute.display()
        ),
    );

    let paths = WebPaths::resolve_from_profile(&profile).unwrap();
    assert_eq!(paths.templates, absolute);
    assert_eq!(paths.assets, services.join("web/assets"));
}

#[test]
fn a_missing_web_config_still_resolves_to_the_defaults() {
    let (profile, services) = fixture();
    let config = services.join("web/config.yaml");
    if config.exists() {
        std::fs::remove_file(&config).unwrap();
    }

    let paths = WebPaths::resolve_from_profile(&profile).unwrap();
    assert_eq!(paths.templates, services.join("web/templates"));
    assert_eq!(paths.assets, services.join("web/assets"));
}

#[test]
fn an_unparseable_web_config_names_the_file_in_the_error() {
    let (profile, services) = fixture();
    write_web_config(&services, "paths: [not a mapping\n");

    let err = WebPaths::resolve_from_profile(&profile).unwrap_err();
    assert!(format!("{err:#}").contains("web/config.yaml"));
}
