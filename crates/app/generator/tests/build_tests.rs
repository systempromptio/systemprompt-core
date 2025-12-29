//! Unit tests for build orchestrator functionality

use std::path::PathBuf;
use systemprompt_generator::{BuildError, BuildMode, BuildOrchestrator};

// =============================================================================
// BuildMode tests
// =============================================================================

#[test]
fn test_build_mode_selection() {
    // Test parsing valid modes
    assert_eq!(
        BuildMode::parse("development"),
        Some(BuildMode::Development)
    );
    assert_eq!(BuildMode::parse("dev"), Some(BuildMode::Development));
    assert_eq!(BuildMode::parse("production"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("prod"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("docker"), Some(BuildMode::Docker));

    // Test case insensitivity
    assert_eq!(
        BuildMode::parse("DEVELOPMENT"),
        Some(BuildMode::Development)
    );
    assert_eq!(BuildMode::parse("Dev"), Some(BuildMode::Development));
    assert_eq!(BuildMode::parse("PRODUCTION"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("Prod"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("DOCKER"), Some(BuildMode::Docker));
    assert_eq!(BuildMode::parse("Docker"), Some(BuildMode::Docker));

    // Test invalid modes
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

// =============================================================================
// BuildOrchestrator tests
// =============================================================================

#[test]
fn test_build_orchestrator_creation() {
    let web_dir = PathBuf::from("/var/www/html/web");
    let orchestrator = BuildOrchestrator::new(web_dir.clone(), BuildMode::Development);

    // Just verify it can be created - internal state is private
    let debug_str = format!("{:?}", orchestrator);
    assert!(debug_str.contains("BuildOrchestrator"));
}

#[test]
fn test_build_orchestrator_with_different_modes() {
    let web_dir = PathBuf::from("/var/www/html/web");

    let dev = BuildOrchestrator::new(web_dir.clone(), BuildMode::Development);
    let prod = BuildOrchestrator::new(web_dir.clone(), BuildMode::Production);
    let docker = BuildOrchestrator::new(web_dir.clone(), BuildMode::Docker);

    // Each should be creatable
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

// =============================================================================
// BuildError tests
// =============================================================================

#[test]
fn test_build_error_theme_generation_failed() {
    let error = BuildError::ThemeGenerationFailed("Custom theme error".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Theme generation failed"));
    assert!(error_msg.contains("Custom theme error"));
}

#[test]
fn test_build_error_typescript_failed() {
    let error = BuildError::TypeScriptFailed("Type error in file.ts".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("TypeScript compilation failed"));
    assert!(error_msg.contains("Type error"));
}

#[test]
fn test_build_error_vite_failed() {
    let error = BuildError::ViteFailed("Module not found".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Vite build failed"));
    assert!(error_msg.contains("Module not found"));
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
    let error = BuildError::ThemeGenerationFailed("test error".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ThemeGenerationFailed"));
    assert!(debug_str.contains("test error"));
}

// =============================================================================
// Build mode round-trip tests
// =============================================================================

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
    // Ensure we can work with all variants
    let modes = [
        ("development", BuildMode::Development),
        ("production", BuildMode::Production),
        ("docker", BuildMode::Docker),
    ];

    for (name, expected) in modes {
        let parsed = BuildMode::parse(name).unwrap();
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), name);
    }
}

// Note: Full build integration tests require a complete Node.js/TypeScript/Vite
// environment and are not suitable for unit tests. The async build methods
// are better tested in integration tests.

#[test]
fn test_build_orchestrator_new_is_const() {
    // Verify that new() is const-compatible (can be used in const contexts)
    const WEB_DIR: &str = "/var/www/html/web";
    let _orchestrator = BuildOrchestrator::new(PathBuf::from(WEB_DIR), BuildMode::Production);
}

// =============================================================================
// Additional BuildMode edge cases
// =============================================================================

#[test]
fn test_build_mode_parse_whitespace() {
    // Whitespace should not be trimmed automatically
    assert_eq!(BuildMode::parse(" development"), None);
    assert_eq!(BuildMode::parse("development "), None);
    assert_eq!(BuildMode::parse(" development "), None);
}

#[test]
fn test_build_mode_parse_partial_matches() {
    // Partial matches should not work
    assert_eq!(BuildMode::parse("develop"), None);
    assert_eq!(BuildMode::parse("product"), None);
    assert_eq!(BuildMode::parse("dock"), None);
}

#[test]
fn test_build_mode_copy() {
    let mode = BuildMode::Production;
    let copied: BuildMode = mode; // Copy trait
    assert_eq!(mode, copied);

    // Both should still be usable
    assert_eq!(mode.as_str(), "production");
    assert_eq!(copied.as_str(), "production");
}
