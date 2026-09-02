//! A replica's identity must be stable across restarts on cloud targets: an
//! explicit `server.instance_id` wins, `HOSTNAME` is accepted, and a random
//! per-boot id is only tolerated for local profiles.

use systemprompt_config::{ConfigError, resolve_instance_id};
use systemprompt_models::Profile;

use crate::fixture;

fn profile(target: &str, instance_id: Option<&str>) -> Profile {
    let dir = tempfile::tempdir().unwrap();
    let mut yaml = fixture::profile_yaml(dir.path(), fixture::FILE_SECRETS, "admin");
    yaml = yaml.replace("target: local", &format!("target: {target}"));
    if let Some(id) = instance_id {
        yaml = yaml.replace("instance_id: null", &format!("instance_id: {id}"));
    }
    serde_yaml::from_str(&yaml).expect("profile yaml parses")
}

#[test]
fn explicit_instance_id_wins() {
    fixture::set_env("HOSTNAME", "host-from-env");
    assert_eq!(
        resolve_instance_id(&profile("cloud", Some("node-a"))).unwrap(),
        "node-a"
    );
    fixture::remove_env("HOSTNAME");
}

#[test]
fn cloud_profile_falls_back_to_hostname() {
    fixture::set_env("HOSTNAME", "host-from-env");
    assert_eq!(
        resolve_instance_id(&profile("cloud", None)).unwrap(),
        "host-from-env"
    );
    fixture::remove_env("HOSTNAME");
}

#[test]
fn cloud_profile_without_any_stable_id_is_refused() {
    fixture::remove_env("HOSTNAME");
    let err = resolve_instance_id(&profile("cloud", None)).unwrap_err();
    assert!(matches!(err, ConfigError::InstanceIdUnresolved), "{err:?}");
}

#[test]
fn local_profile_may_use_a_random_id() {
    fixture::remove_env("HOSTNAME");
    let id = resolve_instance_id(&profile("local", None)).unwrap();
    assert!(id.starts_with("instance-"), "{id}");
}
