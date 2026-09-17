use serde_yaml::{Mapping, Value};
use systemprompt_models::Profile;

use crate::profile_services_sources::local_profile;

fn profile_yaml_without_evaluation() -> Mapping {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    map.remove(Value::String("evaluation".to_owned()));
    map
}

fn parse(map: Mapping) -> Result<Profile, serde_yaml::Error> {
    serde_yaml::from_value(Value::Mapping(map))
}

#[test]
fn absent_block_means_manual_only() {
    let profile = parse(profile_yaml_without_evaluation()).expect("parses without the block");
    assert!(!profile.evaluation.automatic);
}

#[test]
fn automatic_flag_parses() {
    let mut map = profile_yaml_without_evaluation();
    let block: Value = serde_yaml::from_str("automatic: true").unwrap();
    map.insert(Value::String("evaluation".to_owned()), block);
    let profile = parse(map).expect("parses with the block");
    assert!(profile.evaluation.automatic);
}

#[test]
fn unknown_key_is_rejected() {
    let mut map = profile_yaml_without_evaluation();
    let block: Value = serde_yaml::from_str("automatik: true").unwrap();
    map.insert(Value::String("evaluation".to_owned()), block);
    let err = parse(map).expect_err("unknown key rejected");
    assert!(err.to_string().contains("automatik"), "{err}");
}
