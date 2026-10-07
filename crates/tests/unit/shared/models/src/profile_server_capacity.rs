//! `server.max_in_flight`: unset means no ceiling, a positive value parses,
//! and zero, which would refuse every request, is rejected at parse time.

use serde_yaml::Value;
use systemprompt_manifest::Profile;

use crate::profile_services_sources::local_profile;

fn with_server_key(key: &str, value: &str) -> Result<Profile, serde_yaml::Error> {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    let server = map
        .get_mut(Value::String("server".to_owned()))
        .and_then(Value::as_mapping_mut)
        .expect("server block");
    server.insert(
        Value::String(key.to_owned()),
        serde_yaml::from_str(value).expect("value yaml"),
    );
    serde_yaml::from_value(Value::Mapping(map))
}

#[test]
fn unset_means_no_ceiling() {
    assert!(local_profile().server.max_in_flight.is_none());
}

#[test]
fn a_positive_ceiling_parses() {
    let profile = with_server_key("max_in_flight", "256").expect("parses");
    assert_eq!(profile.server.max_in_flight.map(|n| n.get()), Some(256));
}

#[test]
fn zero_is_rejected() {
    let err = with_server_key("max_in_flight", "0").expect_err("zero refused");
    assert!(err.to_string().contains("nonzero"), "{err}");
}
