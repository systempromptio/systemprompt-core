//! Tests for the catalog-file plumbing behind the `admin config` catalog
//! editors: locating `services/ai/`, committing an edit only when the whole
//! tree still loads, and writing `gateway.yaml`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::services_io::{
    CatalogLocation, commit_catalog, write_gateway,
};
use systemprompt_identifiers::{EndpointId, ModelId, ProviderId};
use systemprompt_manifest::services::GatewayConfigSpec;
use systemprompt_manifest::services::catalog::{DefaultCatalog, DeploymentTarget, EndpointRef};

fn tree() -> (tempfile::TempDir, CatalogLocation) {
    let dir = tempfile::tempdir().unwrap();
    let services = dir.path().join("services");
    std::fs::create_dir_all(services.join("config")).unwrap();
    std::fs::create_dir_all(services.join("ai")).unwrap();
    std::fs::write(services.join("config/config.yaml"), "{}\n").unwrap();
    for (name, body) in DefaultCatalog::FILES {
        std::fs::write(services.join("ai").join(name), body).unwrap();
    }
    let location = CatalogLocation::for_root(services.join("config/config.yaml"));
    (dir, location)
}

fn snapshot(location: &CatalogLocation) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(&location.dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    names
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
}

#[test]
fn the_catalog_lives_beside_the_config_directory() {
    let (_dir, location) = tree();
    assert!(
        location.dir.ends_with("services/ai"),
        "{}",
        location.dir.display()
    );
    assert!(location.read().unwrap().models.get("gpt-5").is_some());
}

#[test]
fn a_committed_edit_is_written_and_reloaded() {
    let (_dir, location) = tree();
    let before = location.read().unwrap();
    let mut after = before.clone();
    after.models.models.remove(&ModelId::new("gpt-5"));
    after.deployments.deployments.remove(&ModelId::new("gpt-5"));

    let services = commit_catalog(&location, &after).unwrap();

    assert!(services.models.get("gpt-5").is_none());
    assert!(location.read().unwrap().models.get("gpt-5").is_none());
}

#[test]
fn an_edit_the_loader_refuses_is_rolled_back() {
    let (_dir, location) = tree();
    let before = location.read().unwrap();
    let files = snapshot(&location);
    let mut after = before.clone();
    after.deployments.deployments.insert(
        ModelId::new("gpt-5"),
        vec![DeploymentTarget::Endpoint(EndpointRef {
            provider: ProviderId::new("ghost"),
            endpoint: EndpointId::new("api"),
        })],
    );

    let err = commit_catalog(&location, &after).unwrap_err();

    assert!(err.to_string().contains("no longer loads"), "{err:#}");
    assert_eq!(snapshot(&location), files);
}

#[test]
fn write_gateway_writes_one_gateway_section_and_skips_none() {
    let dir = tempfile::tempdir().unwrap();
    write_gateway(dir.path(), None).unwrap();
    assert!(!dir.path().join("gateway.yaml").exists());

    let spec = GatewayConfigSpec {
        enabled: true,
        default_model: Some(ModelId::new("gpt-5")),
        ..GatewayConfigSpec::default()
    };
    write_gateway(dir.path(), Some(&spec)).unwrap();
    let text = std::fs::read_to_string(dir.path().join("gateway.yaml")).unwrap();
    assert!(text.starts_with("gateway:"), "{text}");
    assert!(text.contains("default_model: gpt-5"), "{text}");
}
