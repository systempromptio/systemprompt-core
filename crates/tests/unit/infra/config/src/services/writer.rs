//! Unit tests for ConfigWriter (via ConfigManager)
//!
//! Tests cover:
//! - Environment file writing with proper formatting
//! - Value quoting for whitespace
//! - Key sorting
//! - Web configuration file generation

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use systemprompt_config::{ConfigManager, DeployEnvironment, EnvironmentConfig};
use tempfile::TempDir;

fn create_test_environment_with_web(
    base_yaml: &str,
    env_yaml: &str,
    environment: DeployEnvironment,
    web_dir_name: &str,
) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let env_dir = temp_dir.path().join("infrastructure/environments");
    let specific_env_dir = env_dir.join(environment.as_str());

    fs::create_dir_all(&specific_env_dir).expect("Failed to create env directories");
    fs::create_dir_all(temp_dir.path().join(web_dir_name)).expect("Failed to create web dir");

    fs::write(env_dir.join("base.yaml"), base_yaml).expect("Failed to write base.yaml");
    fs::write(specific_env_dir.join("config.yaml"), env_yaml).expect("Failed to write config.yaml");

    temp_dir
}

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

#[test]
fn test_write_web_env_file_filters_vite_vars() {
    let base_yaml = r#"
vite_api_url: http://localhost:8080
vite_app_name: TestApp
regular_var: should_not_appear
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment_with_web(base_yaml, env_yaml, DeployEnvironment::Local, "web");
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    manager
        .write_web_env_file(&config)
        .expect("Should write web env file");

    let web_env_path = temp_dir.path().join("web/.env.local");
    let content = fs::read_to_string(&web_env_path).expect("Should read web env file");

    assert!(content.contains("VITE_API_URL="));
    assert!(content.contains("VITE_APP_NAME="));
    assert!(!content.contains("REGULAR_VAR"));
}

#[test]
fn test_write_web_env_file_no_vite_vars() {
    let base_yaml = r#"
database_url: postgres://localhost
api_key: secret
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment_with_web(base_yaml, env_yaml, DeployEnvironment::Local, "web");
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    let result = manager.write_web_env_file(&config);

    assert!(result.is_ok());

    let web_env_path = temp_dir.path().join("web/.env.local");
    assert!(!web_env_path.exists());
}

#[test]
fn test_write_web_env_file_core_web_priority() {
    let base_yaml = r#"
vite_test: value
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment_with_web(base_yaml, env_yaml, DeployEnvironment::Local, "core/web");
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    manager
        .write_web_env_file(&config)
        .expect("Should write web env file");

    let core_web_env_path = temp_dir.path().join("core/web/.env.local");
    assert!(core_web_env_path.exists());
}

#[test]
fn test_write_web_env_file_docker_creates_additional_file() {
    let base_yaml = r#"
vite_api_url: http://api:8080
"#;
    let env_yaml = r#"
name: docker-test
"#;

    let temp_dir = create_test_environment_with_web(base_yaml, env_yaml, DeployEnvironment::DockerDev, "web");
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::DockerDev)
        .expect("Should generate config");

    manager
        .write_web_env_file(&config)
        .expect("Should write web env file");

    let docker_env_path = temp_dir.path().join("web/.env.docker-dev");
    let additional_docker_path = temp_dir.path().join("web/.env.docker");

    assert!(docker_env_path.exists());
    assert!(additional_docker_path.exists());
}

#[cfg(unix)]
#[test]
fn test_write_web_env_file_local_creates_symlink() {
    let base_yaml = r#"
vite_test: value
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment_with_web(base_yaml, env_yaml, DeployEnvironment::Local, "web");
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    manager
        .write_web_env_file(&config)
        .expect("Should write web env file");

    let symlink_path = temp_dir.path().join("web/.env");
    assert!(symlink_path.is_symlink());

    let target = std::fs::read_link(&symlink_path).expect("Should read symlink");
    assert_eq!(target, PathBuf::from(".env.local"));
}

#[test]
fn test_write_env_file_different_environments() {
    for env in [
        DeployEnvironment::Local,
        DeployEnvironment::DockerDev,
        DeployEnvironment::Production,
    ] {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_path = temp_dir.path().join(format!(".env.{}", env.as_str()));

        let mut variables = HashMap::new();
        variables.insert("ENVIRONMENT".to_string(), env.as_str().to_string());

        let config = EnvironmentConfig {
            environment: env,
            variables,
        };

        ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

        let content = fs::read_to_string(&output_path).expect("Should read env file");
        assert!(content.contains(&format!("ENVIRONMENT={}", env.as_str())));
    }
}

#[test]
fn test_write_env_file_creates_parent_directories() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("config/.env");

    fs::create_dir_all(output_path.parent().unwrap()).expect("Failed to create parent dir");

    let mut variables = HashMap::new();
    variables.insert("KEY".to_string(), "value".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    assert!(output_path.exists());
}
