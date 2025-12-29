//! Unit tests for BuildMode and BuildError

use systemprompt_generator::{BuildError, BuildMode};

// ============================================================================
// BuildMode Parsing Tests
// ============================================================================

#[test]
fn test_build_mode_parse_development() {
    assert_eq!(BuildMode::parse("development"), Some(BuildMode::Development));
    assert_eq!(BuildMode::parse("dev"), Some(BuildMode::Development));
}

#[test]
fn test_build_mode_parse_production() {
    assert_eq!(BuildMode::parse("production"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("prod"), Some(BuildMode::Production));
}

#[test]
fn test_build_mode_parse_docker() {
    assert_eq!(BuildMode::parse("docker"), Some(BuildMode::Docker));
}

#[test]
fn test_build_mode_parse_case_insensitive() {
    assert_eq!(BuildMode::parse("DEVELOPMENT"), Some(BuildMode::Development));
    assert_eq!(BuildMode::parse("Development"), Some(BuildMode::Development));
    assert_eq!(BuildMode::parse("PRODUCTION"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("Production"), Some(BuildMode::Production));
    assert_eq!(BuildMode::parse("DOCKER"), Some(BuildMode::Docker));
    assert_eq!(BuildMode::parse("Docker"), Some(BuildMode::Docker));
}

#[test]
fn test_build_mode_parse_invalid() {
    assert_eq!(BuildMode::parse("invalid"), None);
    assert_eq!(BuildMode::parse(""), None);
    assert_eq!(BuildMode::parse("test"), None);
    assert_eq!(BuildMode::parse("staging"), None);
}

// ============================================================================
// BuildMode as_str Tests
// ============================================================================

#[test]
fn test_build_mode_as_str_development() {
    assert_eq!(BuildMode::Development.as_str(), "development");
}

#[test]
fn test_build_mode_as_str_production() {
    assert_eq!(BuildMode::Production.as_str(), "production");
}

#[test]
fn test_build_mode_as_str_docker() {
    assert_eq!(BuildMode::Docker.as_str(), "docker");
}

// ============================================================================
// BuildMode Equality Tests
// ============================================================================

#[test]
fn test_build_mode_equality() {
    assert_eq!(BuildMode::Development, BuildMode::Development);
    assert_eq!(BuildMode::Production, BuildMode::Production);
    assert_eq!(BuildMode::Docker, BuildMode::Docker);
}

#[test]
fn test_build_mode_inequality() {
    assert_ne!(BuildMode::Development, BuildMode::Production);
    assert_ne!(BuildMode::Development, BuildMode::Docker);
    assert_ne!(BuildMode::Production, BuildMode::Docker);
}

// ============================================================================
// BuildMode Clone and Copy Tests
// ============================================================================

#[test]
fn test_build_mode_clone() {
    let mode = BuildMode::Development;
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

#[test]
fn test_build_mode_copy() {
    let mode = BuildMode::Production;
    let copied = mode;
    assert_eq!(mode, copied);
}

// ============================================================================
// BuildError Display Tests
// ============================================================================

#[test]
fn test_build_error_theme_generation_failed() {
    let error = BuildError::ThemeGenerationFailed("invalid config".to_string());
    assert_eq!(error.to_string(), "Theme generation failed: invalid config");
}

#[test]
fn test_build_error_typescript_failed() {
    let error = BuildError::TypeScriptFailed("syntax error on line 42".to_string());
    assert_eq!(
        error.to_string(),
        "TypeScript compilation failed: syntax error on line 42"
    );
}

#[test]
fn test_build_error_vite_failed() {
    let error = BuildError::ViteFailed("module not found".to_string());
    assert_eq!(error.to_string(), "Vite build failed: module not found");
}

#[test]
fn test_build_error_css_organization_failed() {
    let error = BuildError::CssOrganizationFailed("permission denied".to_string());
    assert_eq!(
        error.to_string(),
        "CSS organization failed: permission denied"
    );
}

#[test]
fn test_build_error_validation_failed() {
    let error = BuildError::ValidationFailed("missing index.html".to_string());
    assert_eq!(error.to_string(), "Validation failed: missing index.html");
}

#[test]
fn test_build_error_process_error() {
    let error = BuildError::ProcessError("command exited with code 1".to_string());
    assert_eq!(
        error.to_string(),
        "Process execution error: command exited with code 1"
    );
}

#[test]
fn test_build_error_config_error() {
    let error = BuildError::ConfigError("missing required field".to_string());
    assert_eq!(
        error.to_string(),
        "Configuration error: missing required field"
    );
}

#[test]
fn test_build_error_io_from_std_io_error() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let build_error: BuildError = io_error.into();
    assert!(build_error.to_string().contains("I/O error"));
}

// ============================================================================
// BuildError Debug Tests
// ============================================================================

#[test]
fn test_build_error_debug() {
    let error = BuildError::ValidationFailed("test".to_string());
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("ValidationFailed"));
    assert!(debug_str.contains("test"));
}

// ============================================================================
// BuildMode Debug Tests
// ============================================================================

#[test]
fn test_build_mode_debug() {
    assert!(format!("{:?}", BuildMode::Development).contains("Development"));
    assert!(format!("{:?}", BuildMode::Production).contains("Production"));
    assert!(format!("{:?}", BuildMode::Docker).contains("Docker"));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_build_mode_parse_with_whitespace() {
    // Whitespace should not be trimmed by parse
    assert_eq!(BuildMode::parse(" development"), None);
    assert_eq!(BuildMode::parse("development "), None);
    assert_eq!(BuildMode::parse(" development "), None);
}

#[test]
fn test_build_error_empty_message() {
    let error = BuildError::ValidationFailed(String::new());
    assert_eq!(error.to_string(), "Validation failed: ");
}

#[test]
fn test_build_error_long_message() {
    let long_message = "x".repeat(10000);
    let error = BuildError::ThemeGenerationFailed(long_message.clone());
    assert!(error.to_string().contains(&long_message));
}

#[test]
fn test_build_error_special_characters_in_message() {
    let error = BuildError::ViteFailed("error: <script> tag not allowed".to_string());
    assert!(error.to_string().contains("<script>"));
}
