//! `cloud profile` template writers: secrets file, Dockerfile, catalog default
//! provider and the connection probe.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::profile::api_keys::ApiKeys;
use systemprompt_cli::cloud::profile::templates::{
    DatabaseUrls, get_services_path, save_dockerfile, save_secrets,
    update_ai_config_default_provider, validate_connection,
};
use systemprompt_identifiers::ProfileName;

fn keys() -> ApiKeys {
    ApiKeys {
        gemini: None,
        anthropic: Some("sk-ant".to_owned()),
        openai: None,
    }
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn save_secrets_writes_identity_and_both_database_urls() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("secrets.json");
    save_secrets(
        &DatabaseUrls {
            external: "postgres://u:p@ext:5432/db",
            internal: Some("postgres://u:p@int:5432/db"),
        },
        &keys(),
        &path,
        true,
    )
    .unwrap();
    let json = read_json(&path);
    assert_eq!(json["database_url"], "postgres://u:p@ext:5432/db");
    assert_eq!(json["external_database_url"], "postgres://u:p@ext:5432/db");
    assert_eq!(json["internal_database_url"], "postgres://u:p@int:5432/db");
    assert_eq!(json["anthropic"], "sk-ant");
    assert!(json["gemini"].is_null());
    assert!(!json["signing_key_pem"].as_str().unwrap().is_empty());
    assert!(!json["encryption_master_key"].as_str().unwrap().is_empty());
}

#[test]
fn save_secrets_without_internal_url_omits_the_key_even_when_masked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    save_secrets(
        &DatabaseUrls {
            external: "postgres://u:********@ext:5432/db",
            internal: None,
        },
        &keys(),
        &path,
        false,
    )
    .unwrap();
    let json = read_json(&path);
    assert!(json.get("internal_database_url").is_none());
    assert_eq!(json["database_url"], "postgres://u:********@ext:5432/db");
}

#[test]
fn save_secrets_with_a_masked_internal_url_still_writes_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    save_secrets(
        &DatabaseUrls {
            external: "postgres://u:p@ext:5432/db",
            internal: Some("postgres://u:********@int:5432/db"),
        },
        &keys(),
        &path,
        true,
    )
    .unwrap();
    assert_eq!(
        read_json(&path)["internal_database_url"],
        "postgres://u:********@int:5432/db"
    );
}

#[test]
fn save_dockerfile_names_the_profile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Dockerfile");
    save_dockerfile(&path, &ProfileName::try_new("prod").unwrap(), dir.path()).unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("FROM"));
    assert!(body.contains("prod"));
}

#[test]
fn save_dockerfile_into_a_missing_directory_fails_naming_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("absent").join("Dockerfile");
    let err =
        save_dockerfile(&path, &ProfileName::try_new("prod").unwrap(), dir.path()).unwrap_err();
    assert!(err.to_string().contains("Failed to write"));
}

#[tokio::test]
async fn validate_connection_is_false_for_an_unreachable_database() {
    assert!(!validate_connection("postgres://u:p@127.0.0.1:1/none").await);
}

#[test]
fn update_default_provider_scaffolds_then_rewrites_the_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let services = dir.path().join("services");
    unsafe {
        std::env::set_var("SYSTEMPROMPT_SERVICES_PATH", &services);
    }
    assert_eq!(get_services_path().unwrap(), services.to_string_lossy());

    update_ai_config_default_provider("anthropic").unwrap();
    let ai = services.join("ai");
    assert!(ai.join("config.yaml").exists());
    assert!(ai.join("gateway.yaml").exists());

    update_ai_config_default_provider("anthropic").unwrap();
    let gateway = std::fs::read_to_string(ai.join("gateway.yaml")).unwrap();
    assert!(gateway.contains("default_model"), "{gateway}");

    let before = std::fs::read_to_string(ai.join("gateway.yaml")).unwrap();
    update_ai_config_default_provider("no-such-provider").unwrap();
    assert_eq!(
        std::fs::read_to_string(ai.join("gateway.yaml")).unwrap(),
        before
    );
}
