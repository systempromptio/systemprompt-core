//! `server.max_in_flight`: an omitted key is the default ceiling, an explicit
//! `null` is no ceiling, a positive value parses, and zero, which would refuse
//! every request, is rejected at parse time. `server.max_probe_in_flight`
//! and `server.max_route_body_bytes` default when omitted; a zero probe
//! ceiling and a route-body ceiling of zero or above the hard limit are
//! rejected at parse time.

use serde_yaml::Value;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::{
    DEFAULT_MAX_IN_FLIGHT, DEFAULT_MAX_PROBE_IN_FLIGHT, DEFAULT_MAX_ROUTE_BODY_BYTES,
    MAX_ROUTE_BODY_BYTES_LIMIT,
};

use crate::profile_services_sources::local_profile;

fn server_block(edit: impl FnOnce(&mut serde_yaml::Mapping)) -> Result<Profile, serde_yaml::Error> {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    let server = map
        .get_mut(Value::String("server".to_owned()))
        .and_then(Value::as_mapping_mut)
        .expect("server block");
    edit(server);
    serde_yaml::from_value(Value::Mapping(map))
}

fn with_server_key(key: &str, value: &str) -> Result<Profile, serde_yaml::Error> {
    server_block(|server| {
        server.insert(
            Value::String(key.to_owned()),
            serde_yaml::from_str(value).expect("value yaml"),
        );
    })
}

#[test]
fn an_omitted_ceiling_is_the_default() {
    let profile = server_block(|server| {
        server.remove(Value::String("max_in_flight".to_owned()));
    })
    .expect("parses");
    assert_eq!(profile.server.max_in_flight, Some(DEFAULT_MAX_IN_FLIGHT));
    assert_eq!(DEFAULT_MAX_IN_FLIGHT.get(), 4096);
}

#[test]
fn an_explicit_null_is_no_ceiling() {
    let profile = with_server_key("max_in_flight", "null").expect("parses");
    assert!(profile.server.max_in_flight.is_none());
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

#[test]
fn the_probe_and_route_body_ceilings_default_when_omitted() {
    let profile = server_block(|server| {
        server.remove(Value::String("max_probe_in_flight".to_owned()));
        server.remove(Value::String("max_route_body_bytes".to_owned()));
    })
    .expect("parses");
    assert_eq!(
        profile.server.max_probe_in_flight,
        DEFAULT_MAX_PROBE_IN_FLIGHT
    );
    assert_eq!(
        profile.server.max_route_body_bytes,
        DEFAULT_MAX_ROUTE_BODY_BYTES
    );
}

#[test]
fn explicit_probe_and_route_body_ceilings_parse() {
    let profile = with_server_key("max_probe_in_flight", "8").expect("parses");
    assert_eq!(profile.server.max_probe_in_flight.get(), 8);
    let profile = with_server_key("max_route_body_bytes", "65536").expect("parses");
    assert_eq!(profile.server.max_route_body_bytes, 65_536);
}

#[test]
fn invalid_probe_and_route_body_ceilings_are_rejected() {
    with_server_key("max_probe_in_flight", "0").expect_err("a zero probe ceiling is refused");
    let err = with_server_key("max_route_body_bytes", "0").expect_err("zero refused");
    assert!(err.to_string().contains("max_route_body_bytes"), "{err}");
    let above = (MAX_ROUTE_BODY_BYTES_LIMIT + 1).to_string();
    with_server_key("max_route_body_bytes", &above).expect_err("above the hard limit refused");
}
