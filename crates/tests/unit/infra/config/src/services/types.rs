//! Unit tests for config types
//!
//! Tests cover:
//! - DeployEnvironment enum variants and conversions
//! - DeployEnvironment parsing from strings
//! - DeploymentConfig construction and serialization
//! - EnvironmentConfig creation

use std::collections::HashMap;
use systemprompt_config::{DeployEnvironment, DeploymentConfig, EnvironmentConfig};

// ============================================================================
// DeployEnvironment as_str Tests
// ============================================================================

#[test]
fn test_deploy_environment_local_as_str() {
    let env = DeployEnvironment::Local;
    assert_eq!(env.as_str(), "local");
}

#[test]
fn test_deploy_environment_docker_dev_as_str() {
    let env = DeployEnvironment::DockerDev;
    assert_eq!(env.as_str(), "docker-dev");
}

#[test]
fn test_deploy_environment_production_as_str() {
    let env = DeployEnvironment::Production;
    assert_eq!(env.as_str(), "production");
}

// ============================================================================
// DeployEnvironment parse Tests
// ============================================================================

#[test]
fn test_deploy_environment_parse_local() {
    let result = DeployEnvironment::parse("local");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), DeployEnvironment::Local);
}

#[test]
fn test_deploy_environment_parse_docker() {
    let result = DeployEnvironment::parse("docker");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), DeployEnvironment::DockerDev);
}

#[test]
fn test_deploy_environment_parse_docker_dev() {
    let result = DeployEnvironment::parse("docker-dev");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), DeployEnvironment::DockerDev);
}

#[test]
fn test_deploy_environment_parse_production() {
    let result = DeployEnvironment::parse("production");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), DeployEnvironment::Production);
}

#[test]
fn test_deploy_environment_parse_prod() {
    let result = DeployEnvironment::parse("prod");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), DeployEnvironment::Production);
}

#[test]
fn test_deploy_environment_parse_invalid() {
    let result = DeployEnvironment::parse("invalid");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid environment"));
}

#[test]
fn test_deploy_environment_parse_empty() {
    let result = DeployEnvironment::parse("");
    assert!(result.is_err());
}

#[test]
fn test_deploy_environment_parse_case_sensitive() {
    let result = DeployEnvironment::parse("LOCAL");
    assert!(result.is_err());
}

// ============================================================================
// DeployEnvironment Trait Tests
// ============================================================================

#[test]
fn test_deploy_environment_debug() {
    let env = DeployEnvironment::Local;
    let debug_str = format!("{:?}", env);
    assert!(debug_str.contains("Local"));
}

#[test]
fn test_deploy_environment_clone() {
    let env = DeployEnvironment::Production;
    let cloned = env;
    assert_eq!(env, cloned);
}

#[test]
fn test_deploy_environment_copy() {
    let env = DeployEnvironment::DockerDev;
    let copied = env;
    assert_eq!(env, copied);
}

#[test]
fn test_deploy_environment_eq() {
    assert_eq!(DeployEnvironment::Local, DeployEnvironment::Local);
    assert_ne!(DeployEnvironment::Local, DeployEnvironment::Production);
}

// ============================================================================
// DeploymentConfig Tests
// ============================================================================

#[test]
fn test_deployment_config_new_empty() {
    let config = DeploymentConfig {
        vars: HashMap::new(),
    };
    assert!(config.vars.is_empty());
}

#[test]
fn test_deployment_config_with_string_value() {
    let mut vars = HashMap::new();
    vars.insert("key".to_string(), serde_yaml::Value::String("value".to_string()));
    let config = DeploymentConfig { vars };
    assert_eq!(config.vars.len(), 1);
}

#[test]
fn test_deployment_config_with_multiple_values() {
    let mut vars = HashMap::new();
    vars.insert("str_key".to_string(), serde_yaml::Value::String("value".to_string()));
    vars.insert("num_key".to_string(), serde_yaml::Value::Number(42.into()));
    vars.insert("bool_key".to_string(), serde_yaml::Value::Bool(true));
    let config = DeploymentConfig { vars };
    assert_eq!(config.vars.len(), 3);
}

#[test]
fn test_deployment_config_debug() {
    let config = DeploymentConfig {
        vars: HashMap::new(),
    };
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("DeploymentConfig"));
}

#[test]
fn test_deployment_config_clone() {
    let mut vars = HashMap::new();
    vars.insert("key".to_string(), serde_yaml::Value::String("value".to_string()));
    let config = DeploymentConfig { vars };
    let cloned = config.clone();
    assert_eq!(cloned.vars.len(), 1);
}

#[test]
fn test_deployment_config_serialize() {
    let mut vars = HashMap::new();
    vars.insert("DATABASE_URL".to_string(), serde_yaml::Value::String("postgres://localhost".to_string()));
    let config = DeploymentConfig { vars };
    let yaml = serde_yaml::to_string(&config);
    assert!(yaml.is_ok());
    let yaml_str = yaml.unwrap();
    assert!(yaml_str.contains("DATABASE_URL"));
}

#[test]
fn test_deployment_config_deserialize() {
    let yaml = "DATABASE_URL: postgres://localhost\nPORT: 8080";
    let config: Result<DeploymentConfig, _> = serde_yaml::from_str(yaml);
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.vars.len(), 2);
}

// ============================================================================
// EnvironmentConfig Tests
// ============================================================================

#[test]
fn test_environment_config_new() {
    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables: HashMap::new(),
    };
    assert_eq!(config.environment, DeployEnvironment::Local);
    assert!(config.variables.is_empty());
}

#[test]
fn test_environment_config_with_variables() {
    let mut variables = HashMap::new();
    variables.insert("DATABASE_URL".to_string(), "postgres://localhost".to_string());
    variables.insert("PORT".to_string(), "8080".to_string());
    let config = EnvironmentConfig {
        environment: DeployEnvironment::Production,
        variables,
    };
    assert_eq!(config.environment, DeployEnvironment::Production);
    assert_eq!(config.variables.len(), 2);
    assert_eq!(config.variables.get("PORT"), Some(&"8080".to_string()));
}

#[test]
fn test_environment_config_debug() {
    let config = EnvironmentConfig {
        environment: DeployEnvironment::DockerDev,
        variables: HashMap::new(),
    };
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("EnvironmentConfig"));
    assert!(debug_str.contains("DockerDev"));
}

#[test]
fn test_environment_config_clone() {
    let mut variables = HashMap::new();
    variables.insert("KEY".to_string(), "value".to_string());
    let config = EnvironmentConfig {
        environment: DeployEnvironment::Local,
        variables,
    };
    let cloned = config.clone();
    assert_eq!(cloned.environment, DeployEnvironment::Local);
    assert_eq!(cloned.variables.get("KEY"), Some(&"value".to_string()));
}

#[test]
fn test_environment_config_environment_accessor() {
    let config = EnvironmentConfig {
        environment: DeployEnvironment::Production,
        variables: HashMap::new(),
    };
    assert_eq!(config.environment.as_str(), "production");
}

// ============================================================================
// Integration Between Types
// ============================================================================

#[test]
fn test_environment_config_all_environments() {
    let environments = [
        DeployEnvironment::Local,
        DeployEnvironment::DockerDev,
        DeployEnvironment::Production,
    ];

    for env in environments {
        let config = EnvironmentConfig {
            environment: env,
            variables: HashMap::new(),
        };
        assert_eq!(config.environment, env);
    }
}

#[test]
fn test_parse_roundtrip() {
    let environments = ["local", "docker", "docker-dev", "production", "prod"];

    for env_str in environments {
        let parsed = DeployEnvironment::parse(env_str);
        assert!(parsed.is_ok(), "Failed to parse: {}", env_str);
    }
}
