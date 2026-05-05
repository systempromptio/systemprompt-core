#![allow(clippy::expect_used)]

use std::path::PathBuf;
use systemprompt_generator::{BuildError, BuildMode, BuildOrchestrator};

#[test]
fn test_build_mode_selection() {
    assert_eq!(
        BuildMode::parse("development"),
        Some(BuildMode::Development)
    );
    assert_eq!(BuildMode::parse("dev"), Some(BuildMode::Development));
    assert_eq!(BuildMode::parse("production"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("prod"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("docker"), Some(BuildMode::Docker));

    assert_eq!(
        BuildMode::parse("DEVELOPMENT"),
        Some(BuildMode::Development)
    );
    assert_eq!(BuildMode::parse("Dev"), Some(BuildMode::Development));
    assert_eq!(BuildMode::parse("PRODUCTION"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("Prod"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("DOCKER"), Some(BuildMode::Docker));
    assert_eq!(BuildMode::parse("Docker"), Some(BuildMode::Docker));

    assert_eq!(BuildMode::parse("invalid"), None);
    assert_eq!(BuildMode::parse("test"), None);
    assert_eq!(BuildMode::parse(""), None);
    assert_eq!(BuildMode::parse("staging"), None);
}

#[test]
fn test_build_mode_as_str() {
    assert_eq!(BuildMode::Development.as_str(), "development");
    assert_eq!(BuildMode::Production.as_str(), "production");
    assert_eq!(BuildMode::Docker.as_str(), "docker");
}

#[test]
fn test_build_mode_equality() {
    assert_eq!(BuildMode::Development, BuildMode::Development);
    assert_eq!(BuildMode::Production, BuildMode::Production);
    assert_eq!(BuildMode::Docker, BuildMode::Docker);

    assert_ne!(BuildMode::Development, BuildMode::Production);
    assert_ne!(BuildMode::Production, BuildMode::Docker);
    assert_ne!(BuildMode::Development, BuildMode::Docker);
}

#[test]
fn test_build_mode_clone() {
    let mode = BuildMode::Production;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_build_mode_debug() {
    let mode = BuildMode::Development;
    let debug_str = format!("{:?}", mode);
    assert!(debug_str.contains("Development"));
}

#[test]
fn test_build_orchestrator_creation() {
    let web_dir = PathBuf::from("/var/www/html/web");
    let orchestrator = BuildOrchestrator::new(web_dir, BuildMode::Development);

    let debug_str = format!("{:?}", orchestrator);
    assert!(debug_str.contains("BuildOrchestrator"));
}

#[test]
fn test_build_orchestrator_with_different_modes() {
    let web_dir = PathBuf::from("/var/www/html/web");

    let dev = BuildOrchestrator::new(web_dir.clone(), BuildMode::Development);
    let prod = BuildOrchestrator::new(web_dir.clone(), BuildMode::Production);
    let docker = BuildOrchestrator::new(web_dir, BuildMode::Docker);

    assert!(format!("{:?}", dev).contains("Development"));
    assert!(format!("{:?}", prod).contains("Production"));
    assert!(format!("{:?}", docker).contains("Docker"));
}

#[test]
fn test_build_orchestrator_with_various_paths() {
    let paths = [
        PathBuf::from("/var/www/html/web"),
        PathBuf::from("./relative/path"),
        PathBuf::from("/absolute/path/to/web"),
        PathBuf::from("web"),
    ];

    for path in paths {
        let orchestrator = BuildOrchestrator::new(path.clone(), BuildMode::Production);
        let debug_str = format!("{:?}", orchestrator);
        assert!(debug_str.contains("BuildOrchestrator"));
    }
}

#[test]
fn test_build_error_css_organization_failed() {
    let error = BuildError::CssOrganizationFailed("Failed to copy CSS".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("CSS organization failed"));
}

#[test]
fn test_build_error_validation_failed() {
    let error = BuildError::ValidationFailed("Missing required file".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Validation failed"));
    assert!(error_msg.contains("Missing required file"));
}

#[test]
fn test_build_error_io() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let error = BuildError::Io(io_error);
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("I/O error"));
}

#[test]
fn test_build_error_process_error() {
    let error = BuildError::ProcessError("npm failed with exit code 1".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Process execution error"));
    assert!(error_msg.contains("exit code 1"));
}

#[test]
fn test_build_error_config_error() {
    let error = BuildError::ConfigError("Invalid configuration".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Configuration error"));
    assert!(error_msg.contains("Invalid configuration"));
}

#[test]
fn test_build_error_debug_format() {
    let error = BuildError::ProcessError("test error".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ProcessError"));
    assert!(debug_str.contains("test error"));
}

#[test]
fn test_build_mode_parse_roundtrip() {
    let modes = [
        BuildMode::Development,
        BuildMode::Production,
        BuildMode::Docker,
    ];

    for mode in modes {
        let as_str = mode.as_str();
        let parsed = BuildMode::parse(as_str);
        assert_eq!(parsed, Some(mode));
    }
}

#[test]
fn test_build_mode_all_variants() {
    let modes = [
        ("development", BuildMode::Development),
        ("production", BuildMode::Production),
        ("docker", BuildMode::Docker),
    ];

    for (name, expected) in modes {
        let parsed = BuildMode::parse(name).expect("valid build mode should parse");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), name);
    }
}

#[test]
fn test_build_orchestrator_new_is_const() {
    const WEB_DIR: &str = "/var/www/html/web";
    let _orchestrator = BuildOrchestrator::new(PathBuf::from(WEB_DIR), BuildMode::Production);
}

#[test]
fn test_build_mode_parse_whitespace() {
    assert_eq!(BuildMode::parse(" development"), None);
    assert_eq!(BuildMode::parse("development "), None);
    assert_eq!(BuildMode::parse(" development "), None);
}

#[test]
fn test_build_mode_parse_partial_matches() {
    assert_eq!(BuildMode::parse("develop"), None);
    assert_eq!(BuildMode::parse("product"), None);
    assert_eq!(BuildMode::parse("dock"), None);
}

#[test]
fn test_build_mode_copy() {
    let mode = BuildMode::Production;
    let copied: BuildMode = mode; // Copy trait
    assert_eq!(mode, copied);

    assert_eq!(mode.as_str(), "production");
    assert_eq!(copied.as_str(), "production");
}
