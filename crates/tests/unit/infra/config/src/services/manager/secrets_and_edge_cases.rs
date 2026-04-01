use std::fs;
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
