use systemprompt_models::paths::{PathError, StoragePaths};
use systemprompt_models::profile::PathsConfig;

fn paths_config_with_storage(storage: &str) -> PathsConfig {
    PathsConfig {
        system: "/tmp/system".to_owned(),
        services: "/tmp/services".to_owned(),
        bin: "/tmp/bin".to_owned(),
        web_path: None,
        storage: Some(storage.to_owned()),
        geoip_database: None,
    }
}

#[test]
fn storage_paths_from_profile_ok_when_storage_set() {
    let cfg = paths_config_with_storage("/tmp/storage");
    let sp = StoragePaths::from_profile(&cfg).unwrap();
    assert_eq!(sp.root(), std::path::Path::new("/tmp/storage"));
    assert_eq!(sp.files(), std::path::Path::new("/tmp/storage/files"));
    assert_eq!(sp.exports(), std::path::Path::new("/tmp/storage/exports"));
    assert_eq!(sp.css(), std::path::Path::new("/tmp/storage/files/css"));
    assert_eq!(sp.js(), std::path::Path::new("/tmp/storage/files/js"));
    assert_eq!(sp.fonts(), std::path::Path::new("/tmp/storage/files/fonts"));
    assert_eq!(sp.images(), std::path::Path::new("/tmp/storage/files/images"));
    assert_eq!(
        sp.generated_images(),
        std::path::Path::new("/tmp/storage/files/images/generated")
    );
    assert_eq!(
        sp.logos(),
        std::path::Path::new("/tmp/storage/files/images/logos")
    );
    assert_eq!(sp.audio(), std::path::Path::new("/tmp/storage/files/audio"));
    assert_eq!(sp.video(), std::path::Path::new("/tmp/storage/files/video"));
    assert_eq!(
        sp.documents(),
        std::path::Path::new("/tmp/storage/files/documents")
    );
    assert_eq!(
        sp.uploads(),
        std::path::Path::new("/tmp/storage/files/uploads")
    );
}

#[test]
fn storage_paths_from_profile_errors_when_storage_not_set() {
    let cfg = PathsConfig {
        system: "/tmp/system".to_owned(),
        services: "/tmp/services".to_owned(),
        bin: "/tmp/bin".to_owned(),
        web_path: None,
        storage: None,
        geoip_database: None,
    };
    let err = StoragePaths::from_profile(&cfg).unwrap_err();
    assert!(matches!(
        err,
        PathError::NotConfigured { field: "storage" }
    ));
}

#[test]
fn path_error_not_configured_display() {
    let e = PathError::NotConfigured { field: "web" };
    assert!(e.to_string().contains("web"));
    assert!(e.to_string().contains("Required path not configured"));
}

#[test]
fn path_error_not_found_display() {
    let e = PathError::NotFound {
        path: std::path::PathBuf::from("/nonexistent/path"),
        field: "system",
    };
    assert!(e.to_string().contains("nonexistent"));
}

#[test]
fn path_error_binary_not_found_display() {
    let e = PathError::BinaryNotFound {
        name: "mybinary".to_owned(),
        searched: vec![std::path::PathBuf::from("/usr/bin")],
    };
    assert!(e.to_string().contains("mybinary"));
}

#[test]
fn paths_config_helpers_build_correct_paths() {
    let cfg = paths_config_with_storage("/tmp/storage");
    assert_eq!(cfg.skills(), "/tmp/services/skills");
    assert_eq!(cfg.agents(), "/tmp/services/agents");
    assert_eq!(cfg.hooks(), "/tmp/services/hooks");
    assert_eq!(cfg.plugins(), "/tmp/services/plugins");
    assert_eq!(cfg.marketplaces(), "/tmp/services/marketplaces");
    assert_eq!(cfg.logs(), "/tmp/system/logs");
}

#[test]
fn paths_config_web_path_resolved_uses_web_path_when_set() {
    let mut cfg = paths_config_with_storage("/tmp/storage");
    cfg.web_path = Some("/tmp/custom_web".to_owned());
    assert_eq!(cfg.web_path_resolved(), "/tmp/custom_web");
}

#[test]
fn paths_config_web_path_resolved_defaults_to_system_web() {
    let cfg = paths_config_with_storage("/tmp/storage");
    assert_eq!(cfg.web_path_resolved(), "/tmp/system/web");
}

#[test]
fn paths_config_storage_resolved_some_when_set() {
    let cfg = paths_config_with_storage("/data");
    assert_eq!(cfg.storage_resolved(), Some("/data"));
}

#[test]
fn paths_config_storage_resolved_none_when_unset() {
    let cfg = PathsConfig {
        system: "/s".to_owned(),
        services: "/sv".to_owned(),
        bin: "/b".to_owned(),
        web_path: None,
        storage: None,
        geoip_database: None,
    };
    assert!(cfg.storage_resolved().is_none());
}
