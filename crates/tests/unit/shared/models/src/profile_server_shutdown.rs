//! `server.shutdown`: an absent block keeps the historical windows, a partial
//! block overrides only what it names, unknown keys are refused, and
//! validation rejects windows that would defeat the teardown they bound.

use serde_yaml::Value;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::ShutdownConfig;

use crate::profile_services_sources::local_profile;

fn with_shutdown(block: &str) -> Result<Profile, serde_yaml::Error> {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    let server = map
        .get_mut(Value::String("server".to_owned()))
        .and_then(Value::as_mapping_mut)
        .expect("server block");
    server.insert(
        Value::String("shutdown".to_owned()),
        serde_yaml::from_str(block).expect("block yaml"),
    );
    serde_yaml::from_value(Value::Mapping(map))
}

#[test]
fn absent_block_keeps_the_historical_windows() {
    let shutdown = local_profile().server.shutdown;
    assert_eq!(shutdown, ShutdownConfig::default());
    assert_eq!(shutdown.pre_stop_delay_secs, 0);
    assert_eq!(shutdown.drain_grace_secs, 10);
    assert_eq!(shutdown.child_grace_secs, 5);
    assert_eq!(shutdown.background_grace_secs, 5);
    assert_eq!(shutdown.forced_exit_grace_secs, 10);
    assert!(shutdown.validation_errors().is_empty());
}

#[test]
fn a_partial_block_overrides_only_what_it_names() {
    let profile =
        with_shutdown("{pre_stop_delay_secs: 15, drain_grace_secs: 600}").expect("parses");
    let shutdown = profile.server.shutdown;
    assert_eq!(shutdown.pre_stop_delay_secs, 15);
    assert_eq!(shutdown.drain_grace_secs, 600);
    assert_eq!(shutdown.child_grace_secs, 5);
    assert_eq!(shutdown.total_budget().as_secs(), 15 + 600 + 2 + 10);
    assert!(shutdown.validation_errors().is_empty());
}

#[test]
fn unknown_keys_are_refused() {
    let err = with_shutdown("{drain_grace_ms: 10}").expect_err("unknown key refused");
    assert!(err.to_string().contains("drain_grace_ms"), "{err}");
}

#[test]
fn a_zero_drain_is_rejected() {
    let profile = with_shutdown("{drain_grace_secs: 0}").expect("parses");
    let err = profile
        .validate()
        .expect_err("zero drain refused")
        .to_string();
    assert!(err.contains("drain_grace_secs"), "{err}");
}

#[test]
fn a_backstop_inside_the_teardown_it_bounds_is_rejected() {
    let profile =
        with_shutdown("{child_grace_secs: 30, forced_exit_grace_secs: 30}").expect("parses");
    let err = profile
        .validate()
        .expect_err("backstop refused")
        .to_string();
    assert!(err.contains("forced_exit_grace_secs"), "{err}");
}

#[test]
fn an_unbounded_pre_stop_delay_is_rejected() {
    let profile = with_shutdown("{pre_stop_delay_secs: 3600}").expect("parses");
    let err = profile.validate().expect_err("delay refused").to_string();
    assert!(err.contains("pre_stop_delay_secs"), "{err}");
}
