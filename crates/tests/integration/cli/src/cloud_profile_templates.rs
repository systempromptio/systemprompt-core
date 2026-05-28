use std::path::PathBuf;
use systemprompt_cli::cloud::profile::templates::{
    generate_display_name, generate_oauth_at_rest_pepper, get_services_path, save_dockerignore,
    save_entrypoint, update_ai_config_default_provider, validate_connection,
};
use tempfile::tempdir;

use crate::env_lock;

#[test]
fn save_entrypoint_writes_executable_script() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested").join("entrypoint.sh");
    save_entrypoint(&path).expect("entrypoint should write");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.starts_with("#!/bin/sh"));
    assert!(content.contains("systemprompt"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }
}

#[test]
fn save_dockerignore_writes_known_patterns() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a").join(".dockerignore");
    save_dockerignore(&path).expect("dockerignore should write");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains(".git"));
    assert!(content.contains("target/debug"));
    assert!(content.contains(".env"));
}

#[test]
fn get_services_path_respects_env_override() {
    let _g = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let original = std::env::var("SYSTEMPROMPT_SERVICES_PATH").ok();
    // SAFETY: tests in this crate are not run in parallel against this env var
    // outside of this module; the override is short-lived.
    unsafe {
        std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", &path);
    }
    let resolved = get_services_path().expect("services path should resolve");
    assert_eq!(PathBuf::from(resolved), path);
    unsafe {
        match original {
            Some(v) => std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", v),
            None => std::env::remove_var("SYSTEMPROMPT_SERVICES_PATH"),
        }
    }
}

#[test]
fn update_ai_config_default_provider_creates_file_when_missing() {
    let _g = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let original = std::env::var("SYSTEMPROMPT_SERVICES_PATH").ok();
    unsafe {
        std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", dir.path());
    }
    update_ai_config_default_provider("anthropic").expect("provider update should succeed");
    let ai_yaml = dir.path().join("ai").join("config.yaml");
    assert!(ai_yaml.exists());
    let content = std::fs::read_to_string(&ai_yaml).unwrap();
    assert!(content.contains("anthropic"));
    unsafe {
        match original {
            Some(v) => std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", v),
            None => std::env::remove_var("SYSTEMPROMPT_SERVICES_PATH"),
        }
    }
}

#[test]
fn update_ai_config_default_provider_replaces_existing_value() {
    let _g = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let ai_dir = dir.path().join("ai");
    std::fs::create_dir_all(&ai_dir).unwrap();
    let cfg_path = ai_dir.join("config.yaml");
    std::fs::write(&cfg_path, "default_provider: \"openai\"\nother: value\n").unwrap();
    let original = std::env::var("SYSTEMPROMPT_SERVICES_PATH").ok();
    unsafe {
        std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", dir.path());
    }
    update_ai_config_default_provider("anthropic").expect("update should succeed");
    let content = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(content.contains("default_provider: \"anthropic\""));
    assert!(content.contains("other: value"));
    unsafe {
        match original {
            Some(v) => std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", v),
            None => std::env::remove_var("SYSTEMPROMPT_SERVICES_PATH"),
        }
    }
}

#[tokio::test]
async fn validate_connection_returns_false_for_unreachable_url() {
    // Reserved TEST-NET-1 plus a port unlikely to be listening; will time out
    // or refuse within the 5s budget.
    let ok = validate_connection("postgres://u:p@192.0.2.1:55555/db").await;
    assert!(!ok);
}

#[test]
fn generate_display_name_normalises_input() {
    let name = generate_display_name("hello-world");
    assert!(!name.is_empty());
}

#[test]
fn generate_oauth_at_rest_pepper_produces_unique_values() {
    let a = generate_oauth_at_rest_pepper();
    let b = generate_oauth_at_rest_pepper();
    assert!(!a.is_empty());
    assert!(!b.is_empty());
    assert_ne!(a, b);
}
