//! Tests for env file creation, content format, quoting, sorting, and special characters

use std::collections::HashMap;
use std::fs;
use systemprompt_config::{ConfigManager, DeployEnvironment, EnvironmentConfig};
use tempfile::TempDir;

#[test]
fn test_write_env_file_creates_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("KEY".to_string(), "value".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    assert!(output_path.exists());
}

#[test]
fn test_write_env_file_content_format() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("DATABASE_URL".to_string(), "postgres://localhost".to_string());
    variables.insert("PORT".to_string(), "8080".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("DATABASE_URL=postgres://localhost"));
    assert!(content.contains("PORT=8080"));
}

#[test]
fn test_write_env_file_quotes_whitespace_values() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("DESCRIPTION".to_string(), "hello world".to_string());
    variables.insert("PATH_VAR".to_string(), "/usr/local/bin".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("DESCRIPTION=\"hello world\""));
    assert!(content.contains("PATH_VAR=/usr/local/bin"));
}

#[test]
fn test_write_env_file_quotes_tabs() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("TAB_VALUE".to_string(), "has\ttab".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("TAB_VALUE=\"has\ttab\""));
}

#[test]
fn test_write_env_file_alphabetical_order() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("ZEBRA".to_string(), "last".to_string());
    variables.insert("ALPHA".to_string(), "first".to_string());
    variables.insert("BETA".to_string(), "second".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("ALPHA="));
    assert!(lines[1].starts_with("BETA="));
    assert!(lines[2].starts_with("ZEBRA="));
}

#[test]
fn test_write_env_file_empty_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables: HashMap::new(),
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");
    assert!(content.is_empty());
}

#[test]
fn test_write_env_file_special_characters() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("URL".to_string(), "https://api.example.com/v1?key=abc".to_string());
    variables.insert("REGEX".to_string(), "^[a-zA-Z0-9]+$".to_string());
    variables.insert("JSON".to_string(), "{\"key\":\"value\"}".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("URL=https://api.example.com/v1?key=abc"));
    assert!(content.contains("REGEX=^[a-zA-Z0-9]+$"));
}

#[test]
fn test_write_env_file_equals_in_value() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("CONNECTION_STRING".to_string(), "host=localhost;port=5432".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("CONNECTION_STRING=host=localhost;port=5432"));
}

#[test]
fn test_write_env_file_newlines_in_values() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("MULTILINE".to_string(), "line1\nline2".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("MULTILINE=\"line1\nline2\""));
}

#[test]
fn test_write_env_file_overwrites_existing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    fs::write(&output_path, "OLD_KEY=old_value").expect("Failed to write initial file");

    let mut variables = HashMap::new();
    variables.insert("NEW_KEY".to_string(), "new_value".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(!content.contains("OLD_KEY"));
    assert!(content.contains("NEW_KEY=new_value"));
}

#[test]
fn test_write_env_file_unicode_values() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("GREETING".to_string(), "Hello World".to_string());
    variables.insert("EMOJI".to_string(), "test".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("GREETING=\"Hello World\""));
}

#[test]
fn test_write_env_file_long_values() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let long_value = "x".repeat(1000);

    let mut variables = HashMap::new();
    variables.insert("LONG_KEY".to_string(), long_value.clone());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains(&format!("LONG_KEY={}", long_value)));
}

#[test]
fn test_write_env_file_preserves_empty_string() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("EMPTY".to_string(), String::new());
    variables.insert("NOT_EMPTY".to_string(), "value".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");

    assert!(content.contains("EMPTY="));
    assert!(content.contains("NOT_EMPTY=value"));
}
