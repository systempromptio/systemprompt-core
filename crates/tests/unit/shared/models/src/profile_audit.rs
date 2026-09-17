//! `governance.audit.payload_cap_bytes`: the default when the block is absent,
//! the parsed value when set, the floor the validator enforces, and the
//! `Profile::payload_cap_bytes` accessor the gateway reads.

use serde_yaml::{Mapping, Value};
use systemprompt_models::Profile;
use systemprompt_models::profile::AuditConfig;

use crate::profile_services_sources::local_profile;

fn profile_yaml_without_governance() -> Mapping {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    map.remove(Value::String("governance".to_owned()));
    map
}

fn with_governance(block: &str) -> Result<Profile, serde_yaml::Error> {
    let mut map = profile_yaml_without_governance();
    let block: Value = serde_yaml::from_str(block).expect("governance yaml");
    map.insert(Value::String("governance".to_owned()), block);
    serde_yaml::from_value(Value::Mapping(map))
}

#[test]
fn default_cap_is_one_mib() {
    assert_eq!(AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES, 1024 * 1024);
    assert_eq!(
        AuditConfig::default().payload_cap_bytes,
        AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES
    );
}

#[test]
fn absent_governance_block_yields_the_default_cap() {
    let profile: Profile =
        serde_yaml::from_value(Value::Mapping(profile_yaml_without_governance()))
            .expect("parses without governance");
    assert!(profile.governance.is_none());
    assert_eq!(
        profile.payload_cap_bytes(),
        AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES
    );
}

#[test]
fn governance_without_audit_yields_the_default_cap() {
    let profile = with_governance("authz: null").expect("parses");
    assert_eq!(
        profile.payload_cap_bytes(),
        AuditConfig::DEFAULT_PAYLOAD_CAP_BYTES
    );
}

#[test]
fn configured_cap_parses() {
    let profile = with_governance("audit:\n  payload_cap_bytes: 4194304").expect("parses");
    assert_eq!(profile.payload_cap_bytes(), 4 * 1024 * 1024);
}

#[test]
fn unknown_audit_key_is_rejected() {
    let err = with_governance("audit:\n  payload_cap: 1").expect_err("unknown key rejected");
    assert!(err.to_string().contains("payload_cap"), "{err}");
}

#[test]
fn cap_below_the_floor_fails_validation() {
    let profile = with_governance("audit:\n  payload_cap_bytes: 1024").expect("parses");
    let err = profile.validate().expect_err("64 KiB floor");
    assert!(
        err.to_string()
            .contains("governance.audit.payload_cap_bytes must be at least 65536"),
        "{err}"
    );
}

#[test]
fn cap_at_the_floor_passes_validation() {
    let profile = with_governance(&format!(
        "audit:\n  payload_cap_bytes: {}",
        AuditConfig::MIN_PAYLOAD_CAP_BYTES
    ))
    .expect("parses");
    profile.validate().expect("floor is inclusive");
}
