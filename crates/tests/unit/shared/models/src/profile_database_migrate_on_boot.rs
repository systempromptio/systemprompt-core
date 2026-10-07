//! `database.migrate_on_boot`: on unless a profile turns it off, so an
//! existing profile keeps migrating at boot.

use serde_yaml::Value;
use systemprompt_manifest::Profile;

use crate::profile_services_sources::local_profile;

fn with_database_block(block: &str) -> Profile {
    let Value::Mapping(mut map) = serde_yaml::to_value(local_profile()).expect("profile to yaml")
    else {
        panic!("profile serialises to a mapping");
    };
    map.insert(
        Value::String("database".to_owned()),
        serde_yaml::from_str(block).expect("database yaml"),
    );
    serde_yaml::from_value(Value::Mapping(map)).expect("profile parses")
}

#[test]
fn omitted_means_migrate_on_boot() {
    let profile = with_database_block("type: postgres");
    assert!(profile.database.migrate_on_boot);
}

#[test]
fn false_round_trips() {
    let profile = with_database_block("type: postgres\nmigrate_on_boot: false");
    assert!(!profile.database.migrate_on_boot);

    let yaml = serde_yaml::to_string(&profile).expect("serialise");
    let reparsed: Profile = serde_yaml::from_str(&yaml).expect("reparse");
    assert!(!reparsed.database.migrate_on_boot);
}
