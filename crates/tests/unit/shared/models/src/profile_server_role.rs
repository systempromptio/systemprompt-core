//! `server.role`: the retired `admin` role is refused naming its replacements,
//! a `worker` must advertise an address its `api` peers can reach, and the
//! scaled roles need storage every replica shares.

use serde_yaml::Value;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::{NodeRole, StorageBackend, StorageConfig};

use crate::profile_services_sources::local_profile;
use crate::profile_validation::{errors_of, valid_profile};

fn with_role(role: &str) -> Result<Profile, serde_yaml::Error> {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    let server = map
        .get_mut(Value::String("server".to_owned()))
        .and_then(Value::as_mapping_mut)
        .expect("server block");
    server.insert(
        Value::String("role".to_owned()),
        Value::String(role.to_owned()),
    );
    serde_yaml::from_value(Value::Mapping(map))
}

fn shared_storage() -> StorageConfig {
    StorageConfig {
        shared: true,
        ..StorageConfig::default()
    }
}

#[test]
fn role_admin_fails_naming_api_and_worker() {
    let err = with_role("admin")
        .expect_err("admin is retired")
        .to_string();
    assert!(err.contains("'api'"), "{err}");
    assert!(err.contains("'worker'"), "{err}");
}

#[test]
fn api_and_worker_parse() {
    assert_eq!(with_role("api").expect("api").server.role, NodeRole::Api);
    assert_eq!(
        with_role("worker").expect("worker").server.role,
        NodeRole::Worker
    );
}

#[test]
fn the_default_all_role_needs_no_advertise_url_or_shared_storage() {
    let p = valid_profile();
    assert_eq!(p.server.role, NodeRole::All);
    let errors = errors_of(&p);
    assert!(!errors.contains("server.role"), "{errors}");
    assert!(!errors.contains("advertise_url"), "{errors}");
}

#[test]
fn a_worker_without_advertise_url_is_rejected() {
    let mut p = valid_profile();
    p.server.role = NodeRole::Worker;
    p.storage = shared_storage();
    assert!(errors_of(&p).contains("server.advertise_url"));
}

#[test]
fn a_worker_with_an_http_advertise_url_is_accepted() {
    let mut p = valid_profile();
    p.server.role = NodeRole::Worker;
    p.storage = shared_storage();
    p.server.advertise_url = Some("http://10.0.0.7:8080".to_owned());
    let errors = errors_of(&p);
    assert!(!errors.contains("advertise_url"), "{errors}");
    assert!(!errors.contains("server.role"), "{errors}");
}

#[test]
fn a_relative_advertise_url_is_rejected() {
    let mut p = valid_profile();
    p.server.advertise_url = Some("worker:8080".to_owned());
    assert!(errors_of(&p).contains("absolute http(s) URL"));
}

#[test]
fn scaled_roles_refuse_unshared_local_storage() {
    for role in [NodeRole::Api, NodeRole::Worker] {
        let mut p = valid_profile();
        p.server.role = role;
        p.server.advertise_url = Some("http://10.0.0.7:8080".to_owned());
        assert_eq!(p.storage.backend, StorageBackend::Local);
        assert!(!p.storage.shared);
        assert!(
            errors_of(&p).contains("storage.shared: true"),
            "{role} must refuse unshared local storage"
        );
        p.storage = shared_storage();
        assert!(!errors_of(&p).contains("storage.shared: true"));
    }
}
