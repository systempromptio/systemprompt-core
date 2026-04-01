use std::collections::HashMap;
use std::fs;
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
