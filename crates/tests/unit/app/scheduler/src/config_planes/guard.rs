use std::sync::Arc;

use systemprompt_manifest::profile::ConfigPlanesConfig;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_provider_contracts::{ConfigPlane, ConfigPlaneError};
use systemprompt_runtime::{ConfigPlaneSet, EffectiveAuthority};

use super::fake_plane::{FakePlane, unique_plane_name};
use super::plane_set;

#[test]
fn a_declared_plane_defaults_to_file_managed_and_refuses_db_writes() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("guard_file"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let set = plane_set(root.path(), &[Arc::clone(&plane)], &[]);

    assert_eq!(
        set.authority(plane.name).expect("authority"),
        EffectiveAuthority::File
    );
    assert!(set.is_file_managed(plane.name).expect("managed"));
    let err = set.ensure_db_writable(plane.name).expect_err("refused");
    assert!(matches!(err, ConfigPlaneError::FileManaged { ref path, .. } if path == plane.path));
}

#[test]
fn a_db_authority_plane_accepts_db_writes_even_with_a_file() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("guard_db"), &[]));
    plane.write_declaration(root.path(), &[("a", "1")]);
    let set = plane_set(root.path(), &[Arc::clone(&plane)], &[plane.name]);

    assert_eq!(
        set.authority(plane.name).expect("authority"),
        EffectiveAuthority::Db
    );
    set.ensure_db_writable(plane.name).expect("writable");
}

#[test]
fn a_file_authority_plane_without_its_file_is_undeclared_and_writable() {
    let root = tempfile::tempdir().expect("tempdir");
    let plane = Arc::new(FakePlane::new(unique_plane_name("guard_absent"), &[]));
    let set = plane_set(root.path(), &[Arc::clone(&plane)], &[]);

    assert_eq!(
        set.authority(plane.name).expect("authority"),
        EffectiveAuthority::Undeclared
    );
    set.ensure_db_writable(plane.name).expect("writable");
}

#[test]
fn an_unknown_plane_is_a_typed_error() {
    let root = tempfile::tempdir().expect("tempdir");
    let set = plane_set(root.path(), &[], &[]);
    assert!(matches!(
        set.ensure_db_writable("nope"),
        Err(ConfigPlaneError::UnknownPlane(name)) if name == "nope"
    ));
}

#[test]
fn duplicate_plane_names_are_refused() {
    let name = unique_plane_name("guard_dup");
    let planes: Vec<Arc<dyn ConfigPlane>> = vec![
        Arc::new(FakePlane::new(name, &[])),
        Arc::new(FakePlane::new(name, &[])),
    ];
    let result = ConfigPlaneSet::new(planes, ConfigPlanesConfig::default(), "/tmp".into());
    assert!(matches!(result, Err(ConfigPlaneError::DuplicatePlane(n)) if n == name));
}

#[test]
fn profile_keys_without_a_registered_plane_are_listed() {
    let root = tempfile::tempdir().expect("tempdir");
    let set = plane_set(root.path(), &[], &["ghost_plane"]);
    assert_eq!(set.unregistered_authority_keys(), vec!["ghost_plane"]);
}

#[test]
fn file_managed_refusal_maps_to_409_with_a_stable_key() {
    let api: ApiError = ConfigPlaneError::FileManaged {
        plane: "groups".into(),
        path: "access/groups.yaml".into(),
    }
    .into();
    assert!(matches!(api.code, ErrorCode::ConflictError));
    assert_eq!(api.error_key.as_deref(), Some("config_plane_file_managed"));

    let unknown: ApiError = ConfigPlaneError::UnknownPlane("x".into()).into();
    assert!(matches!(unknown.code, ErrorCode::NotFound));
}
