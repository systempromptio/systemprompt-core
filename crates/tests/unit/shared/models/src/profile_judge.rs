use serde_yaml::{Mapping, Value};
use systemprompt_models::Profile;

use crate::profile_services_sources::local_profile;

// Why "judge" and not "evaluation": the field is `pub judge` with
// `alias = "evaluation"`, and an alias is a deserialisation-only courtesy —
// serde still serialises the key as `judge`. Removing "evaluation" here was a
// no-op, so `absent_block_means_manual_only` was asserting against whatever
// the local profile happened to carry rather than against an absent block.
fn profile_yaml_without_judge() -> Mapping {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    map.remove(Value::String("judge".to_owned()));
    map
}

fn parse(map: Mapping) -> Result<Profile, serde_yaml::Error> {
    serde_yaml::from_value(Value::Mapping(map))
}

#[test]
fn absent_block_means_manual_only() {
    let profile = parse(profile_yaml_without_judge()).expect("parses without the block");
    assert!(!profile.judge.automatic);
}

// Inserts the pre-5c8d3ae9e key on purpose: this is the regression test for
// the `alias = "evaluation"` that keeps a deployed profile.yaml booting.
#[test]
fn automatic_flag_parses() {
    let mut map = profile_yaml_without_judge();
    let block: Value = serde_yaml::from_str("automatic: true").unwrap();
    map.insert(Value::String("evaluation".to_owned()), block);
    let profile = parse(map).expect("parses with the block");
    assert!(profile.judge.automatic);
}

#[test]
fn unknown_key_is_rejected() {
    let mut map = profile_yaml_without_judge();
    let block: Value = serde_yaml::from_str("automatik: true").unwrap();
    map.insert(Value::String("evaluation".to_owned()), block);
    let err = parse(map).expect_err("unknown key rejected");
    assert!(err.to_string().contains("automatik"), "{err}");
}
