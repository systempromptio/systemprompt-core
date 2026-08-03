//! Tests for the local-database docker-compose scaffolding.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::setup::docker_compose::{
    create_compose_files_if_missing, is_compose_available, is_container_running,
    is_docker_available,
};

#[test]
fn compose_scaffolding_writes_the_service_and_init_script() {
    let tmp = tempfile::tempdir().unwrap();
    let compose_dir = tmp.path().join("infrastructure/docker");

    create_compose_files_if_missing(&compose_dir, "cov-postgres", 55_432).unwrap();

    let compose = std::fs::read_to_string(compose_dir.join("docker-compose.yaml")).unwrap();
    assert!(compose.contains("container_name: cov-postgres"));
    assert!(compose.contains("\"55432:5432\""));
    assert!(compose.contains("cov-postgres_data:/var/lib/postgresql"));
    assert!(compose.contains("cov-postgres_network"));

    let init = std::fs::read_to_string(compose_dir.join("init-scripts/01-extensions.sql")).unwrap();
    assert!(init.contains("uuid-ossp"));
    assert!(init.contains("pgcrypto"));
}

#[test]
fn compose_scaffolding_rewrites_an_existing_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let compose_dir = tmp.path().join("docker");
    std::fs::create_dir_all(&compose_dir).unwrap();
    std::fs::write(compose_dir.join("docker-compose.yaml"), "stale: true\n").unwrap();

    create_compose_files_if_missing(&compose_dir, "second-pass", 6543).unwrap();

    let compose = std::fs::read_to_string(compose_dir.join("docker-compose.yaml")).unwrap();
    assert!(!compose.contains("stale"));
    assert!(compose.contains("container_name: second-pass"));
}

#[test]
fn docker_probes_return_without_panicking() {
    // Probe results depend on whether a docker binary exists on the host, so
    // only their agreement with each other is assertable.
    let compose = is_compose_available();
    if compose {
        assert!(is_docker_available());
    }
    let running = is_container_running("cov-definitely-absent-container");
    assert!(!running);
}
