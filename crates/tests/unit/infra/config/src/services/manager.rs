//! Unit tests for ConfigManager
//!
//! Tests cover:
//! - ConfigManager construction
//! - Configuration generation and variable resolution
//! - Environment file writing
//! - Error handling for missing files

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use systemprompt_config::{ConfigManager, DeployEnvironment, EnvironmentConfig};
use tempfile::TempDir;

fn create_test_environment(
    base_yaml: &str,
    env_yaml: &str,
    environment: DeployEnvironment,
) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let env_dir = temp_dir.path().join("infrastructure/environments");
    let specific_env_dir = env_dir.join(environment.as_str());

    fs::create_dir_all(&specific_env_dir).expect("Failed to create env directories");

    fs::write(env_dir.join("base.yaml"), base_yaml).expect("Failed to write base.yaml");
    fs::write(specific_env_dir.join("config.yaml"), env_yaml).expect("Failed to write config.yaml");

    temp_dir
}

#[test]
fn test_config_manager_new() {
    let path = PathBuf::from("/test/project");
    let manager = ConfigManager::new(path.clone());
    let debug_str = format!("{:?}", manager);
    assert!(debug_str.contains("ConfigManager"));
}

#[test]
fn test_generate_config_missing_base_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let result = manager.generate_config(DeployEnvironment::Local);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Base config not found"));
}

#[test]
fn test_generate_config_missing_env_config() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let env_dir = temp_dir.path().join("infrastructure/environments");
    fs::create_dir_all(&env_dir).expect("Failed to create env dir");
    fs::write(env_dir.join("base.yaml"), "service_name: test").expect("Failed to write base.yaml");

    let manager = ConfigManager::new(temp_dir.path().to_path_buf());
    let result = manager.generate_config(DeployEnvironment::Local);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Environment config not found"));
}

#[test]
fn test_generate_config_simple() {
    let base_yaml = r#"
service_name: test-service
database:
  url: postgresql://localhost/test
"#;
    let env_yaml = r#"
host: 127.0.0.1
port: 8080
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let result = manager.generate_config(DeployEnvironment::Local);
    assert!(result.is_ok());

    let config = result.unwrap();
    assert_eq!(config.environment, DeployEnvironment::Local);
    assert_eq!(
        config.variables.get("SERVICE_NAME"),
        Some(&"test-service".to_string())
    );
    assert_eq!(
        config.variables.get("DATABASE_URL"),
        Some(&"postgresql://localhost/test".to_string())
    );
    assert_eq!(
        config.variables.get("HOST"),
        Some(&"127.0.0.1".to_string())
    );
    assert_eq!(config.variables.get("PORT"), Some(&"8080".to_string()));
}

#[test]
fn test_generate_config_env_overrides_base() {
    let base_yaml = r#"
port: 3000
debug: false
"#;
    let env_yaml = r#"
port: 8080
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(config.variables.get("PORT"), Some(&"8080".to_string()));
    assert_eq!(config.variables.get("DEBUG"), Some(&"false".to_string()));
}

#[test]
fn test_generate_config_variable_resolution() {
    let base_yaml = r#"
host: localhost
port: 8080
api_url: http://${HOST}:${PORT}
"#;
    let env_yaml = r#"
service_name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("API_URL"),
        Some(&"http://localhost:8080".to_string())
    );
}

#[test]
fn test_generate_config_variable_with_default() {
    let base_yaml = r#"
timeout: ${EXTERNAL_TIMEOUT:-30}
retries: ${EXTERNAL_RETRIES:-3}
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(config.variables.get("TIMEOUT"), Some(&"30".to_string()));
    assert_eq!(config.variables.get("RETRIES"), Some(&"3".to_string()));
}

#[test]
fn test_generate_config_nested_yaml_flattening() {
    let base_yaml = r#"
database:
  host: localhost
  port: 5432
  name: mydb
server:
  host: 0.0.0.0
  port: 8080
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("DATABASE_HOST"),
        Some(&"localhost".to_string())
    );
    assert_eq!(
        config.variables.get("DATABASE_PORT"),
        Some(&"5432".to_string())
    );
    assert_eq!(
        config.variables.get("DATABASE_NAME"),
        Some(&"mydb".to_string())
    );
    assert_eq!(
        config.variables.get("SERVER_HOST"),
        Some(&"0.0.0.0".to_string())
    );
    assert_eq!(
        config.variables.get("SERVER_PORT"),
        Some(&"8080".to_string())
    );
}

#[test]
fn test_generate_config_deep_nesting() {
    let base_yaml = r#"
app:
  database:
    primary:
      host: primary.db.local
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("APP_DATABASE_PRIMARY_HOST"),
        Some(&"primary.db.local".to_string())
    );
}

#[test]
fn test_generate_config_boolean_values() {
    let base_yaml = r#"
debug: true
production: false
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(config.variables.get("DEBUG"), Some(&"true".to_string()));
    assert_eq!(
        config.variables.get("PRODUCTION"),
        Some(&"false".to_string())
    );
}

#[test]
fn test_generate_config_numeric_values() {
    let base_yaml = r#"
port: 8080
timeout: 30
rate_limit: 1.5
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(config.variables.get("PORT"), Some(&"8080".to_string()));
    assert_eq!(config.variables.get("TIMEOUT"), Some(&"30".to_string()));
    assert_eq!(config.variables.get("RATE_LIMIT"), Some(&"1.5".to_string()));
}

#[test]
fn test_generate_config_different_environments() {
    for env in [
        DeployEnvironment::Local,
        DeployEnvironment::DockerDev,
        DeployEnvironment::Production,
    ] {
        let base_yaml = "name: base";
        let env_yaml = format!("environment: {}", env.as_str());

        let temp_dir = create_test_environment(base_yaml, &env_yaml, env);
        let manager = ConfigManager::new(temp_dir.path().to_path_buf());

        let config = manager
            .generate_config(env)
            .expect("Should generate config");

        assert_eq!(config.environment, env);
        assert_eq!(
            config.variables.get("ENVIRONMENT"),
            Some(&env.as_str().to_string())
        );
    }
}

#[test]
fn test_generate_config_chained_variable_resolution() {
    let base_yaml = r#"
host: localhost
port: 8080
base_url: http://${HOST}:${PORT}
api_endpoint: ${BASE_URL}/api
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("BASE_URL"),
        Some(&"http://localhost:8080".to_string())
    );
    assert_eq!(
        config.variables.get("API_ENDPOINT"),
        Some(&"http://localhost:8080/api".to_string())
    );
}

#[test]
fn test_write_env_file_simple() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("HOST".to_string(), "localhost".to_string());
    variables.insert("PORT".to_string(), "8080".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");
    assert!(content.contains("HOST=localhost"));
    assert!(content.contains("PORT=8080"));
}

#[test]
fn test_write_env_file_sorted_keys() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("ZEBRA".to_string(), "z".to_string());
    variables.insert("APPLE".to_string(), "a".to_string());
    variables.insert("MIDDLE".to_string(), "m".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");
    let lines: Vec<&str> = content.lines().collect();

    assert!(lines[0].starts_with("APPLE="));
    assert!(lines[1].starts_with("MIDDLE="));
    assert!(lines[2].starts_with("ZEBRA="));
}

#[test]
fn test_write_env_file_quotes_whitespace() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join(".env");

    let mut variables = HashMap::new();
    variables.insert("MESSAGE".to_string(), "hello world".to_string());
    variables.insert("SIMPLE".to_string(), "nowhitespace".to_string());

    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };

    ConfigManager::write_env_file(&config, &output_path).expect("Should write env file");

    let content = fs::read_to_string(&output_path).expect("Should read env file");
    assert!(content.contains("MESSAGE=\"hello world\""));
    assert!(content.contains("SIMPLE=nowhitespace"));
}

#[test]
fn test_write_env_file_empty_variables() {
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
fn test_generate_config_with_secrets_file() {
    let base_yaml = r#"
secret_value: ${TEST_SECRET:-default_secret}
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);

    fs::write(temp_dir.path().join(".env.secrets"), "TEST_SECRET=super_secret\n")
        .expect("Failed to write secrets");

    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("SECRET_VALUE"),
        Some(&"super_secret".to_string())
    );
}

#[test]
fn test_generate_config_secrets_with_quotes() {
    let base_yaml = r#"
password: ${DB_PASSWORD:-default}
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);

    fs::write(
        temp_dir.path().join(".env.secrets"),
        "DB_PASSWORD=\"quoted_password\"\n",
    )
    .expect("Failed to write secrets");

    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("PASSWORD"),
        Some(&"quoted_password".to_string())
    );
}

#[test]
fn test_generate_config_secrets_with_comments() {
    let base_yaml = r#"
api_key: ${API_KEY:-none}
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);

    let secrets_content = r#"
# This is a comment
API_KEY=real_key

# Another comment
"#;
    fs::write(temp_dir.path().join(".env.secrets"), secrets_content)
        .expect("Failed to write secrets");

    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("API_KEY"),
        Some(&"real_key".to_string())
    );
}

#[test]
fn test_generate_config_no_secrets_file() {
    let base_yaml = r#"
value: ${MISSING:-default}
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(config.variables.get("VALUE"), Some(&"default".to_string()));
}

#[test]
fn test_generate_config_invalid_yaml() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let env_dir = temp_dir.path().join("infrastructure/environments/local");
    fs::create_dir_all(&env_dir).expect("Failed to create directories");

    fs::write(
        temp_dir
            .path()
            .join("infrastructure/environments/base.yaml"),
        "invalid: yaml: content:",
    )
    .expect("Failed to write base.yaml");

    fs::write(env_dir.join("config.yaml"), "name: test").expect("Failed to write config.yaml");

    let manager = ConfigManager::new(temp_dir.path().to_path_buf());
    let result = manager.generate_config(DeployEnvironment::Local);

    assert!(result.is_err());
}

#[test]
fn test_generate_config_uppercase_conversion() {
    let base_yaml = r#"
myLowerCaseKey: value1
my_snake_case: value2
MixedCase: value3
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("MYLOWERCASEKEY"),
        Some(&"value1".to_string())
    );
    assert_eq!(
        config.variables.get("MY_SNAKE_CASE"),
        Some(&"value2".to_string())
    );
    assert_eq!(
        config.variables.get("MIXEDCASE"),
        Some(&"value3".to_string())
    );
}

#[test]
fn test_generate_config_empty_string_values() {
    let base_yaml = r#"
empty_value: ""
normal_value: test
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("NORMAL_VALUE"),
        Some(&"test".to_string())
    );
}

#[test]
fn test_generate_config_special_characters_in_values() {
    let base_yaml = r#"
url: https://example.com/path?query=1&other=2
regex: "^[a-z]+$"
"#;
    let env_yaml = r#"
name: test
"#;

    let temp_dir = create_test_environment(base_yaml, env_yaml, DeployEnvironment::Local);
    let manager = ConfigManager::new(temp_dir.path().to_path_buf());

    let config = manager
        .generate_config(DeployEnvironment::Local)
        .expect("Should generate config");

    assert_eq!(
        config.variables.get("URL"),
        Some(&"https://example.com/path?query=1&other=2".to_string())
    );
    assert_eq!(
        config.variables.get("REGEX"),
        Some(&"^[a-z]+$".to_string())
    );
}
