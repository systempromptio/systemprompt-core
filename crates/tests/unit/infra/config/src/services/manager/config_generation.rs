use std::fs;
use std::path::PathBuf;
use systemprompt_config::{ConfigManager, DeployEnvironment};
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
