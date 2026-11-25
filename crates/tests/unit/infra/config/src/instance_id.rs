//! A replica's identity must be stable across restarts on cloud targets: an
//! explicit `server.instance_id` wins, `HOSTNAME` is accepted, and a random
//! local profile with neither resolves one id per profile and machine, so the
//! host and a CLI on the same machine share it and another machine never does.

use systemprompt_config::{ConfigError, resolve_instance_id};
use systemprompt_manifest::Profile;
use systemprompt_manifest::config::{MachineIdentity, local_instance_id};

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
fn local_profile_without_an_id_resolves_one_id_per_profile() {
    fixture::remove_env("HOSTNAME");
    let local = profile("local", None);
    let host = resolve_instance_id(&local).unwrap();
    let cli = resolve_instance_id(&local).unwrap();
    assert_eq!(host, cli);
    let machine = MachineIdentity::probe().expect("this machine has an identity");
    assert_eq!(host.as_str(), format!("local-{}-{machine}", local.name));
}

#[test]
fn another_machine_never_resolves_the_same_local_id() {
    let local = profile("local", None);
    let here = MachineIdentity::from_source("machine-a").unwrap();
    let there = MachineIdentity::from_source("machine-b").unwrap();
    assert_eq!(here, MachineIdentity::from_source(" machine-a\n").unwrap());
    assert_ne!(
        local_instance_id(&local, &here),
        local_instance_id(&local, &there)
    );
    assert!(MachineIdentity::from_source("  ").is_none());
    assert_eq!(here.as_str().len(), 12);
}

#[test]
fn blank_instance_id_is_treated_as_absent() {
    fixture::set_env("HOSTNAME", "host-from-env");
    let profile = profile("cloud", Some("\"   \""));
    assert!(profile.server.instance_id.is_none());
    assert_eq!(resolve_instance_id(&profile).unwrap(), "host-from-env");
    fixture::remove_env("HOSTNAME");
}

#[test]
fn explicit_instance_id_is_trimmed_at_load() {
    let profile = profile("cloud", Some("\" node-a \""));
    assert_eq!(
        profile.server.instance_id.as_ref().map(|id| id.as_str()),
        Some("node-a")
    );
}
