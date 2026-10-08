//! Unit tests for BuildMode and BuildError

use systemprompt_generator::{BuildError, BuildMode};

#[test]
fn test_build_mode_parse_development() {
    assert_eq!(
        BuildMode::parse("development"),
        Some(BuildMode::Development)
    );
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
    assert_eq!(
        BuildMode::parse("DEVELOPMENT"),
        Some(BuildMode::Development)
    );
    assert_eq!(
        BuildMode::parse("Development"),
        Some(BuildMode::Development)
    );
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

#[test]
fn test_build_error_css_organization_failed() {
    let error = BuildError::CssOrganizationFailed {
        context: "Failed to copy content.css to css/".to_string(),
        source: std::io::Error::other("permission denied"),
    };
    assert_eq!(
        error.to_string(),
        "CSS organization failed: Failed to copy content.css to css/: permission denied"
    );
}

#[test]
fn test_build_error_validation_failed() {
    let error = BuildError::MissingIndex {
        path: std::path::PathBuf::from("/dist/index.html"),
    };
    assert_eq!(
        error.to_string(),
        "Validation failed: index.html not found at /dist/index.html"
    );
}

#[test]
fn test_build_error_io_from_std_io_error() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let build_error: BuildError = io_error.into();
    assert!(build_error.to_string().contains("I/O error"));
}


#[test]
fn test_build_mode_parse_with_whitespace() {
    assert_eq!(BuildMode::parse(" development"), None);
    assert_eq!(BuildMode::parse("development "), None);
    assert_eq!(BuildMode::parse(" development "), None);
}

#[test]
fn test_build_error_empty_url() {
    let error = BuildError::InvalidSitemapUrl { url: String::new() };
    assert_eq!(
        error.to_string(),
        "Validation failed: invalid sitemap URL format: "
    );
}

#[test]
fn test_build_error_long_message() {
    let long_message = "x".repeat(10000);
    let error = BuildError::CssOrganizationFailed {
        context: long_message.clone(),
        source: std::io::Error::other("io"),
    };
    assert!(error.to_string().contains(&long_message));
}

#[test]
fn test_build_error_special_characters_in_message() {
    let error = BuildError::InvalidSitemapUrl {
        url: "<script>".to_string(),
    };
    assert!(error.to_string().contains("<script>"));
}
