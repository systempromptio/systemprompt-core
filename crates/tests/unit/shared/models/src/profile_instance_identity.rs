//! `server.instance_id` against the profile target: a cloud replica takes
//! its identity from HOSTNAME, so a static id there is refused.

use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::ProfileType;

use crate::profile_services_sources::{errors_of, local_profile};

const REFUSAL: &str = "cloud profiles derive the replica identity from HOSTNAME";

#[test]
fn cloud_profile_with_a_static_instance_id_is_rejected() {
    let mut profile = local_profile();
    profile.target = ProfileType::Cloud;
    profile.server.instance_id = Some(InstanceId::new("replica-1"));

    let errors = errors_of(&profile);
    assert!(errors.contains(REFUSAL), "{errors}");
    assert!(errors.contains("remove server.instance_id"), "{errors}");
}

#[test]
fn cloud_profile_without_an_instance_id_passes_the_identity_check() {
    let mut profile = local_profile();
    profile.target = ProfileType::Cloud;
    profile.server.instance_id = None;

    assert!(!errors_of(&profile).contains(REFUSAL));
}

#[test]
fn local_profile_may_pin_an_instance_id() {
    let mut profile = local_profile();
    profile.server.instance_id = Some(InstanceId::new("dev-node"));

    let errors = errors_of(&profile);
    assert!(!errors.contains(REFUSAL), "{errors}");
    assert!(errors.is_empty(), "{errors}");
}
