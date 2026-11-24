//! `database.migrate_on_boot`: on unless a profile turns it off, so an
//! existing profile keeps migrating at boot. `database.schema_install_roles`
//! narrows the roles that install; omitted, every role installs.

use serde_yaml::Value;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::NodeRole;

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

#[test]
fn omitted_install_roles_let_every_role_install() {
    let profile = with_database_block("type: postgres");
    assert!(profile.database.schema_install_roles.is_none());
    for role in [
        NodeRole::All,
        NodeRole::Api,
        NodeRole::Gateway,
        NodeRole::Worker,
        NodeRole::Runner,
    ] {
        assert!(profile.database.installs_on_boot(role), "{role:?} installs");
    }
}

#[test]
fn listed_install_roles_are_the_only_installers() {
    let profile = with_database_block("type: postgres\nschema_install_roles: [all, api]");
    assert!(profile.database.installs_on_boot(NodeRole::All));
    assert!(profile.database.installs_on_boot(NodeRole::Api));
    assert!(!profile.database.installs_on_boot(NodeRole::Worker));
    assert!(!profile.database.installs_on_boot(NodeRole::Gateway));

    let yaml = serde_yaml::to_string(&profile).expect("serialise");
    let reparsed: Profile = serde_yaml::from_str(&yaml).expect("reparse");
    assert_eq!(
        reparsed.database.schema_install_roles,
        Some(vec![NodeRole::All, NodeRole::Api])
    );
}

#[test]
fn migrate_on_boot_false_wins_over_a_listed_role() {
    let profile =
        with_database_block("type: postgres\nmigrate_on_boot: false\nschema_install_roles: [all]");
    assert!(!profile.database.installs_on_boot(NodeRole::All));
}
