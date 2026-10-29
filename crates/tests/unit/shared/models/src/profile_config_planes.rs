use systemprompt_manifest::profile::{ConfigPlanesConfig, PlaneAuthority};

use crate::profile_validation::{errors_of, valid_profile};

#[test]
fn unnamed_planes_default_to_db_authority() {
    let profile = valid_profile();
    assert_eq!(
        profile.config_planes.authority_for("groups"),
        PlaneAuthority::Db
    );
}

#[test]
fn a_plane_named_file_is_file_authority_and_validates() {
    let mut profile = valid_profile();
    profile
        .config_planes
        .authority
        .insert("identity".to_string(), PlaneAuthority::File);
    assert_eq!(
        profile.config_planes.authority_for("identity"),
        PlaneAuthority::File
    );
    assert!(errors_of(&profile).is_empty());
}

#[test]
fn a_plane_named_db_is_db_authority_and_validates() {
    let mut profile = valid_profile();
    profile
        .config_planes
        .authority
        .insert("access_control".to_string(), PlaneAuthority::Db);
    assert_eq!(
        profile.config_planes.authority_for("access_control"),
        PlaneAuthority::Db
    );
    assert!(errors_of(&profile).is_empty());
}

#[test]
fn a_malformed_plane_name_fails_validation() {
    let mut profile = valid_profile();
    profile
        .config_planes
        .authority
        .insert("Gateway-Routes".to_string(), PlaneAuthority::Db);
    assert!(errors_of(&profile).contains("config_planes.authority: 'Gateway-Routes'"));
}

#[test]
fn authority_parses_from_yaml_and_rejects_unknown_values() {
    let parsed: ConfigPlanesConfig =
        serde_yaml::from_str("authority:\n  groups: db\n  governance: file\n").expect("parse");
    assert_eq!(parsed.authority_for("groups"), PlaneAuthority::Db);
    assert_eq!(parsed.authority_for("governance"), PlaneAuthority::File);
    assert!(serde_yaml::from_str::<ConfigPlanesConfig>("authority:\n  groups: git\n").is_err());
}
