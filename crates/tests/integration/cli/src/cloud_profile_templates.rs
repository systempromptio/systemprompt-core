use std::path::PathBuf;
use systemprompt_cli::cloud::profile::templates::{
    get_services_path, save_dockerignore, save_entrypoint, update_ai_config_default_provider,
    validate_connection,
};
use systemprompt_cli::shared::profile::generate_oauth_at_rest_pepper;
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
    let ai = dir.path().join("ai");
    for file in [
        "config.yaml",
        "credentials.yaml",
        "providers.yaml",
        "models.yaml",
        "deployments.yaml",
    ] {
        assert!(ai.join(file).exists(), "{file} seeded");
    }
    let gateway = std::fs::read_to_string(ai.join("gateway.yaml")).unwrap();
    assert!(gateway.contains("default_model: claude-"), "{gateway}");
    unsafe {
        match original {
            Some(v) => std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", v),
            None => std::env::remove_var("SYSTEMPROMPT_SERVICES_PATH"),
        }
    }
}

#[test]
fn update_ai_config_default_provider_moves_the_default_model() {
    let _g = env_lock::ENV.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let ai_dir = dir.path().join("ai");
    std::fs::create_dir_all(&ai_dir).unwrap();
    for (name, body) in systemprompt_manifest::services::catalog::DefaultCatalog::FILES {
        std::fs::write(ai_dir.join(name), body).unwrap();
    }
    let original = std::env::var("SYSTEMPROMPT_SERVICES_PATH").ok();
    unsafe {
        std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", dir.path());
    }
    update_ai_config_default_provider("anthropic").expect("update should succeed");
    let gateway = std::fs::read_to_string(ai_dir.join("gateway.yaml")).unwrap();
    assert!(gateway.contains("default_model: claude-"), "{gateway}");
    update_ai_config_default_provider("openai").expect("update should succeed");
    let gateway = std::fs::read_to_string(ai_dir.join("gateway.yaml")).unwrap();
    assert!(!gateway.contains("default_model: claude-"), "{gateway}");
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
fn generate_oauth_at_rest_pepper_produces_unique_values() {
    let a = generate_oauth_at_rest_pepper();
    let b = generate_oauth_at_rest_pepper();
    assert!(!a.is_empty());
    assert!(!b.is_empty());
    assert_ne!(a, b);
}
